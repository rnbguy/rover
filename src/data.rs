use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chain {
    pub name: String,
    pub chain_id: String,
    pub denom: String,
    pub bech32_prefix: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee: Option<u64>,

    pub address: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub grantee: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rest_endpoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rpc_endpoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grpc_endpoints: Vec<String>,

    pub broadcast: Broadcast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Broadcast {
    Rpc,
    Grpc,
    Rest,
}
