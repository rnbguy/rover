use cosmos_sdk_proto::cosmos::base::abci::v1beta1::TxResponse;
use cosmos_sdk_proto::cosmos::tx::v1beta1::{
    service_client::ServiceClient, BroadcastTxRequest, BroadcastTxResponse,
};
use cosmos_sdk_proto::cosmos::tx::v1beta1::{BroadcastMode, SimulateRequest, SimulateResponse, Tx};

use cosmos_sdk_proto::prost_wkt_types::MessageSerde;

use crate::Result;

use serde::Serialize;
use tendermint::abci::Path;
use tendermint_rpc::endpoint::broadcast::tx_sync::Response as TendermintResponse;
use tendermint_rpc::{Client, HttpClient};

use std::str::FromStr;

pub fn create_broadcast_sync_payload(tx: &Tx) -> Result<BroadcastTxRequest> {
    Ok(BroadcastTxRequest {
        tx_bytes: tx.try_encoded()?,
        mode: BroadcastMode::Sync.into(),
    })
}

pub async fn broadcast_via_rest(endpoint: &str, signed_tx: &Tx) -> Result<TxResponse> {
    let broadcast_req = create_broadcast_sync_payload(signed_tx)?;
    let url = format!("{endpoint}/cosmos/tx/v1beta1/txs");
    Ok(serde_json::from_value(
        ureq::post(&url).send_json(broadcast_req)?.into_json()?,
    )?)
}

pub async fn broadcast_via_tendermint_rpc(
    endpoint: &str,
    signed_tx: &Tx,
) -> Result<TendermintResponse> {
    let rpc_client = HttpClient::new(endpoint).unwrap();

    Ok(rpc_client
        .broadcast_tx_sync(signed_tx.try_encoded()?.into())
        .await?)
}

pub async fn broadcast_via_grpc(endpoint: &str, signed_tx: Tx) -> Result<BroadcastTxResponse> {
    let broadcast_req = create_broadcast_sync_payload(&signed_tx)?;
    let mut service_client = ServiceClient::connect(endpoint.to_owned()).await?;
    Ok(service_client
        .broadcast_tx(broadcast_req)
        .await?
        .into_inner())
}

pub async fn simulate_via_grpc(endpoint: &str, tx: Tx) -> Result<SimulateResponse> {
    let sim_req = SimulateRequest {
        tx_bytes: tx.try_encoded()?,
        ..Default::default()
    };
    let mut service_client = ServiceClient::connect(endpoint.to_owned()).await?;
    Ok(service_client.simulate(sim_req).await?.into_inner())
}

pub async fn simulate_via_tendermint_rpc<Q>(endpoint: &str, query: Q) -> Result<u64>
where
    Q: Serialize + MessageSerde,
{
    let rpc_client = tendermint_rpc::HttpClient::new(endpoint).unwrap();

    let resp = rpc_client
        .abci_query(
            Some(Path::from_str("/app/simulate")?),
            query.try_encoded()?,
            None,
            false,
        )
        .await?;

    dbg!(&resp.log);
    dbg!(String::from_utf8_lossy(&resp.value));

    let val: serde_json::Value = serde_json::from_slice(&resp.value)?;

    Ok(val
        .pointer("/gas_info/gas_used")
        .unwrap()
        .as_str()
        .unwrap()
        .parse()
        .unwrap())
}
