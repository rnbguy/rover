use anyhow::Context;
use futures::future::join_all;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use url::Url;

use crate::query::{validate_grpc, validate_rpc};
use crate::Result;

#[derive(Debug, Deserialize)]
pub struct ZoneNodes {
    pub zone_nodes: Vec<RpcAddress>,
}

fn str_to_url<'de, D>(deserializer: D) -> std::result::Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let a: &str = Deserialize::deserialize(deserializer)?;
    a.try_into().map_err(D::Error::custom)
}

#[derive(Debug, Deserialize)]
pub struct RpcAddress {
    pub zone: String,
    #[serde(deserialize_with = "str_to_url")]
    pub rpc_addr: Url,
}

#[derive(Serialize)]
pub struct Vars {
    id: String,
}

pub async fn get_rpc_endpoints<'a>(
    chain_id: &'a str,
    graphql_endpoint: &str,
) -> Result<Vec<(u64, String)>> {
    let query = r#"
        query Query($id: String!) {
            zone_nodes(where: {zone: {_eq: $id}, is_alive: {_eq: true}}, order_by: {last_checked_at: desc})
            {zone rpc_addr}
        }
   "#;

    let client = gql_client::Client::new(graphql_endpoint);
    let vars = Vars {
        id: chain_id.into(),
    };
    let data = client
        .query_with_vars::<ZoneNodes, Vars>(query, vars)
        .await
        .expect("parse error")
        .expect("none error");

    let mut list = join_all(data.zone_nodes.into_iter().map(|rpc_struct| async {
        let rpc = rpc_struct.rpc_addr;
        let height = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            validate_rpc(rpc.as_str(), chain_id),
        )
        .await??;
        Result::Ok((height, rpc.to_string().trim_end_matches('/').into()))
    }))
    .await
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    list.sort_unstable_by(|a, b| b.cmp(a));

    Ok(list)
}

pub async fn get_cosmos_directory_name(chain_id: &str) -> Result<String> {
    let resp: Value = ureq::get("https://chains.cosmos.directory")
        .call()?
        .into_json()?;
    let chain_name = resp
        .pointer("/chains")
        .context("no chains value")?
        .as_array()
        .context("no vec value")?
        .iter()
        .filter_map(|v| (v.pointer("/chain_id")?.as_str()? == chain_id).then_some(v))
        .map(|v| {
            v.pointer("/name")
                .and_then(|k| k.as_str())
                .context("name in chains data")
        })
        .next()
        .context("at least one chain")??;

    Ok(format!("https://rpc.cosmos.directory/{chain_name}"))
}

pub async fn get_zone_ids<'a>(graphql_endpoint: &str) -> Result<Vec<String>> {
    let query = r#"
        query Query {
            zone_nodes(where: {is_alive: {_eq: true}}, order_by: {zone: asc}, distinct_on: zone)
            {zone rpc_addr}
        }
   "#;

    let client = gql_client::Client::new(graphql_endpoint);
    let data = client
        .query::<ZoneNodes>(query)
        .await
        .expect("parse error")
        .expect("none error");

    Ok(data.zone_nodes.into_iter().map(|x| x.zone).collect())
}

pub async fn transform_to_grpc_endpoint(rpc_addr: &str) -> Result<String> {
    let mut grpc_addr = Url::try_from(rpc_addr)?;
    grpc_addr.set_port(Some(9090)).expect("error");
    grpc_addr.set_scheme("http").expect("error");
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        validate_grpc(grpc_addr.as_str()),
    )
    .await??;
    Ok(grpc_addr
        .to_string()
        .trim_end_matches('/')
        .trim_start_matches("http://")
        .into())
}
