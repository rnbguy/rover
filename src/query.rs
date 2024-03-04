use anyhow::Context;
use cosmos_sdk_proto::cosmos::bank::v1beta1::{
    query_client::QueryClient as QueryTotalSupplyClient, QueryTotalSupplyRequest,
    QueryTotalSupplyResponse,
};
use cosmos_sdk_proto::cosmos::base::query::v1beta1::PageRequest;
use cosmos_sdk_proto::cosmos::vesting::v1beta1::ContinuousVestingAccount;
use cosmos_sdk_proto::prost_wkt_types::{Any, MessageSerde};

use cosmos_sdk_proto::cosmos::auth::v1beta1::{
    BaseAccount, QueryAccountRequest, QueryAccountResponse, QueryAccountsRequest,
    QueryAccountsResponse,
};

use serde_json::Value;
use tracing::info;

use crate::endpoint::get_rpc_endpoints;
use crate::Result;

use tendermint::abci::response::Info;
use tendermint_rpc::endpoint::status::Response as NodeStatus;
use tendermint_rpc::Client;

use futures::stream::StreamExt;

pub async fn perform_rpc_query<S, R>(endpoint: &str, query: S) -> Result<R>
where
    S: MessageSerde + Default,
    R: MessageSerde + Default + Clone,
{
    let query_any = Any::try_pack(query)?;

    let type_url = query_any.type_url;
    let type_url = type_url.replace(".Query", ".Query/");
    let type_url = type_url.replace("Request", "");

    let pb_data = query_any.value;

    let rpc_client = tendermint_rpc::HttpClient::new(endpoint)?;

    let resp = rpc_client
        .abci_query(Some(type_url), pb_data, None, false)
        .await?;

    crate::utils::read_from_bytes(&resp.value)
}

pub async fn get_account_info(endpoint: &str, address: &str) -> Result<QueryAccountResponse> {
    let query = QueryAccountRequest {
        address: address.into(),
    };
    perform_rpc_query(endpoint, query).await
}

pub async fn get_total_supply_grpc(endpoint: &str) -> Result<QueryTotalSupplyResponse> {
    let query = QueryTotalSupplyRequest::default();
    let mut cl = QueryTotalSupplyClient::connect(endpoint.to_owned()).await?;
    Ok(cl.total_supply(query).await?.into_inner())
}

pub async fn validate_rpc(endpoint: &str, chain_id: &str) -> Result<u64> {
    let rpc_client = tendermint_rpc::HttpClient::new(endpoint)?;
    let resp = rpc_client.status().await?;
    info!("[RPC] {:?}", resp);
    (chain_id == resp.node_info.network.as_str())
        .then(|| resp.sync_info.latest_block_height.value())
        .context(format!(
            "chain-id didn't match, {} vs {}",
            chain_id,
            resp.node_info.network.as_str()
        ))
}

pub async fn get_chain_id_rpc(endpoint: &str) -> Result<String> {
    let rpc_client = tendermint_rpc::HttpClient::new(endpoint)?;
    let resp = rpc_client.status().await?;
    info!("[RPC] {:?}", resp);
    Ok(resp.node_info.network.as_str().into())
}

pub async fn validate_grpc(endpoint: &str) -> Result<()> {
    let resp = get_total_supply_grpc(endpoint).await?;
    info!("[GRPC validate] {:?}", resp);
    Ok(())
}

pub async fn validate_rest(endpoint: String) -> Result<()> {
    let url = format!("{endpoint}/cosmos/bank/v1beta1/total_supply");
    let resp: Value = ureq::get(&url).call()?.into_json()?;
    info!("[REST] {:?}", resp);
    Ok(())
}

pub async fn get_node_info_rpc(endpoint: &str) -> Result<(NodeStatus, Info)> {
    let rpc_client = tendermint_rpc::HttpClient::new(endpoint)?;

    Ok((rpc_client.status().await?, rpc_client.abci_info().await?))
}

pub async fn get_balance(address: &str, endpoint: &str) -> Result<Vec<(String, u128)>> {
    let q = cosmos_sdk_proto::cosmos::bank::v1beta1::QueryAllBalancesRequest {
        address: address.into(),
        ..Default::default()
    };

    let resp: cosmos_sdk_proto::cosmos::bank::v1beta1::QueryAllBalancesResponse =
        perform_rpc_query(endpoint, q).await?;

    info!("[Balance] {:?}", resp);

    resp.balances
        .into_iter()
        .filter(|c| !c.denom.contains('/'))
        .map(|c| Ok((c.denom, c.amount.parse()?)))
        .collect::<Result<Vec<_>>>()
}

pub async fn get_delegated(address: &str, endpoint: &str) -> Result<Vec<(String, u128)>> {
    let q = cosmos_sdk_proto::cosmos::staking::v1beta1::QueryDelegatorDelegationsRequest {
        delegator_addr: address.into(),
        ..Default::default()
    };

    let resp: cosmos_sdk_proto::cosmos::staking::v1beta1::QueryDelegatorDelegationsResponse =
        perform_rpc_query(endpoint, q).await?;

    info!("[Delegated] {:?}", resp);

    resp.delegation_responses
        .into_iter()
        .map(|c| {
            Ok((
                c.delegation.context("no delegation")?.validator_address,
                c.balance.context("no balance")?.amount.parse()?,
            ))
        })
        .collect::<Result<Vec<_>>>()
}

pub async fn get_rewards(address: &str, endpoint: &str) -> Result<Vec<(String, u128)>> {
    let q = cosmos_sdk_proto::cosmos::distribution::v1beta1::QueryDelegationTotalRewardsRequest {
        delegator_address: address.into(),
    };

    let resp: cosmos_sdk_proto::cosmos::distribution::v1beta1::QueryDelegationTotalRewardsResponse =
        perform_rpc_query(endpoint, q).await?;

    info!("[Rewards] {:?}", resp);

    resp.total
        .into_iter()
        .map(|c| Ok((c.denom, crate::utils::parse_dec_amount(&c.amount, 18)?)))
        .collect::<Result<Vec<_>>>()
}

pub async fn get_chain_id_info(
    chain_id: &str,
    graphql_endpoint: &str,
) -> Result<crate::chain::Chain> {
    futures::stream::iter(get_rpc_endpoints(chain_id, graphql_endpoint).await?)
        .then(|(_, rpc_endpoint)| async move { get_rpc_endpoint_chain_info(&rpc_endpoint).await })
        .filter_map(|x| async { x.ok() })
        .boxed_local()
        .next()
        .await
        .context("no good endpoint found to parse zone data")
}

pub async fn get_rpc_endpoint_chain_info(rpc_endpoint: &str) -> Result<crate::chain::Chain> {
    let chain_id = get_chain_id_rpc(rpc_endpoint).await?;

    let page_request = PageRequest {
        limit: 1,
        ..Default::default()
    };
    let q = QueryAccountsRequest {
        pagination: Some(page_request),
    };

    let resp: QueryAccountsResponse = perform_rpc_query(rpc_endpoint, q).await?;

    let account_any = &resp.accounts[0];
    let address = account_any
        .clone()
        .unpack_as(BaseAccount::default())
        .or_else(|_| {
            account_any
                .clone()
                .unpack_as(ContinuousVestingAccount::default())
                .map(|x| x.base_vesting_account.unwrap().base_account.unwrap())
        })?
        .address;
    let (prefix, _) = bech32::decode(&address)?;

    let page_request = PageRequest {
        limit: u64::MAX,
        ..Default::default()
    };

    let q = QueryTotalSupplyRequest {
        pagination: Some(page_request),
    };

    let resp: QueryTotalSupplyResponse = perform_rpc_query(rpc_endpoint, q).await?;

    let denoms = resp
        .supply
        .iter()
        .filter(|x| !x.denom.contains('/'))
        .map(|x| &x.denom)
        .collect::<Vec<_>>();

    Result::Ok(if denoms.len() == 1 {
        crate::chain::Chain {
            chain_id,
            prefix: prefix.to_string(),
            fee: 0,
            denom: denoms[0].into(),
        }
    } else {
        let question = requestty::Question::select(&format!("choose denom for {chain_id}"))
            .message(&format!("choose denom for {chain_id}"))
            .choices(denoms)
            .build();

        let denom = requestty::prompt_one(question)?
            .as_list_item()
            .expect("answer")
            .text
            .clone();

        crate::chain::Chain {
            chain_id,
            prefix: prefix.to_string(),
            fee: 0,
            denom,
        }
    })
}
