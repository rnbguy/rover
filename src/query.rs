use cosmos_sdk_proto::cosmos::auth::v1beta1::QueryAccountResponse;

use cosmos_sdk_proto::cosmos::auth::v1beta1::{
    query_client::QueryClient as QueryAccountClient, QueryAccountRequest,
};

use cosmos_sdk_proto::cosmos::bank::v1beta1::{
    query_client::QueryClient as QueryTotalSupplyClient, QueryTotalSupplyRequest,
};

use tracing::info;

use crate::Result;

use tendermint_rpc::endpoint::abci_info::AbciInfo;
use tendermint_rpc::endpoint::status::Response as NodeStatus;
use tendermint_rpc::Client;

pub async fn get_account_info(endpoint: &str, address: &str) -> Result<QueryAccountResponse> {
    let q = QueryAccountRequest {
        address: address.into(),
    };
    let mut client = QueryAccountClient::connect(endpoint.to_owned()).await?;
    Ok(client.account(q).await?.into_inner())
}

pub async fn validate_rpc(endpoint: &str) -> Result<()> {
    let rpc_client = tendermint_rpc::HttpClient::new(endpoint).unwrap();
    let resp = rpc_client.status().await?;
    info!("[RPC] {:?}", resp);
    Ok(())
}

pub async fn validate_grpc_old(endpoint: &str, _bech32: &str) -> Result<()> {
    let total_supply = QueryTotalSupplyRequest::default();
    let mut cl = QueryTotalSupplyClient::connect(endpoint.to_owned()).await?;
    let resp = cl.total_supply(total_supply).await?.into_inner();
    info!("[GRPC validate] {:?}", resp);
    Ok(())
}

pub async fn validate_grpc(endpoint: &str, bech32: &str) -> Result<()> {
    let figment_cosmos_address = "cosmos1hjct6q7npsspsg3dgvzk3sdf89spmlpfg8wwf7";
    let figment_chain_addres = crate::utils::bech32(figment_cosmos_address, bech32)?;
    let resp = get_account_info(endpoint, &figment_chain_addres).await?;
    info!("[GRPC validate] {:?}", resp);
    Ok(())
}

pub async fn validate_rest(endpoint: String) -> Result<()> {
    let url = format!("{endpoint}/cosmos/bank/v1beta1/total_supply");
    let resp = ureq::get(&url).call()?.into_json()?;
    info!("[REST] {:?}", resp);
    Ok(())
}

pub async fn get_node_info_rpc(endpoint: &str) -> Result<(NodeStatus, AbciInfo)> {
    let rpc_client = tendermint_rpc::HttpClient::new(endpoint).unwrap();

    Ok((rpc_client.status().await?, rpc_client.abci_info().await?))
}

pub async fn get_balance(address: &str, endpoint: &str) -> Result<Vec<(String, u64)>> {
    let q = cosmos_sdk_proto::cosmos::bank::v1beta1::QueryAllBalancesRequest {
        address: address.into(),
        ..Default::default()
    };

    let mut cl = cosmos_sdk_proto::cosmos::bank::v1beta1::query_client::QueryClient::connect(
        endpoint.to_owned(),
    )
    .await?;

    let resp = cl.all_balances(q).await?.into_inner();

    info!("[Balance] {:?}", resp);

    resp.balances
        .into_iter()
        .filter(|c| !c.denom.contains('/'))
        .map(|c| Ok((c.denom, c.amount.parse()?)))
        .collect::<Result<Vec<_>>>()
}

pub async fn get_delegated(address: &str, endpoint: &str) -> Result<Vec<(String, u64)>> {
    let q = cosmos_sdk_proto::cosmos::staking::v1beta1::QueryDelegatorDelegationsRequest {
        delegator_addr: address.into(),
        ..Default::default()
    };

    let mut cl = cosmos_sdk_proto::cosmos::staking::v1beta1::query_client::QueryClient::connect(
        endpoint.to_owned(),
    )
    .await?;

    let resp = cl.delegator_delegations(q).await?.into_inner();

    info!("[Delegated] {:?}", resp);

    resp.delegation_responses
        .into_iter()
        .map(|c| {
            Ok((
                c.delegation.unwrap().validator_address,
                c.balance.unwrap().amount.parse()?,
            ))
        })
        .collect::<Result<Vec<_>>>()
}

pub async fn get_rewards(address: &str, endpoint: &str) -> Result<Vec<(String, u64)>> {
    let q = cosmos_sdk_proto::cosmos::distribution::v1beta1::QueryDelegationTotalRewardsRequest {
        delegator_address: address.into(),
    };

    let mut cl =
        cosmos_sdk_proto::cosmos::distribution::v1beta1::query_client::QueryClient::connect(
            endpoint.to_owned(),
        )
        .await?;

    let resp = cl.delegation_total_rewards(q).await?.into_inner();

    info!("[Rewards] {:?}", resp);

    resp.total
        .into_iter()
        .map(|c| Ok((c.denom, crate::utils::parse_dec_amount(&c.amount, 18)?)))
        .collect::<Result<Vec<_>>>()
}
