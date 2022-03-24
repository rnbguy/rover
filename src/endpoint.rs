use crate::{
    query::{validate_grpc, validate_rpc},
    Result,
};

use futures::{stream::BoxStream, StreamExt};
use serde::{de::Error, Deserialize, Deserializer, Serialize};

use url::Url;

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
    #[serde(deserialize_with = "str_to_url")]
    pub rpc_addr: Url,
}

#[derive(Serialize)]
pub struct Vars {
    id: String,
}

pub struct Endpoint {
    pub rpc: String,
    pub grpc: String,
}

pub async fn get_endpoints<'a>(
    chain_id: &'a str,
    bech32_prefix: &'a str,
    graphql_endpoint: &str,
) -> Result<BoxStream<'a, Endpoint>> {
    let query = r#"
        query UserByIdQuery($id: String!) {
            zone_nodes(where: {zone: {_eq: $id}, is_alive: {_eq: true}}, order_by: {last_checked_at: desc})
            {rpc_addr}
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

    Ok(futures::stream::iter(data.zone_nodes)
        .then(|mut grpc| async {
            let rpc = grpc.rpc_addr.clone();
            grpc.rpc_addr.set_port(Some(9090)).expect("error");
            grpc.rpc_addr.set_scheme("http").expect("error");
            let grpc = grpc.rpc_addr;
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                validate_grpc(grpc.as_str(), bech32_prefix),
            )
            .await??;
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                validate_rpc(rpc.as_str()),
            )
            .await??;
            Result::Ok(Endpoint {
                rpc: rpc.to_string().strip_suffix('/').expect("error").to_owned(),
                grpc: grpc
                    .to_string()
                    .strip_suffix('/')
                    .expect("error")
                    .to_owned(),
            })
        })
        .filter_map(|x| async { x.ok() })
        .boxed())
}
