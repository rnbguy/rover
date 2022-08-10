use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Chain {
    pub chain_id: String,
    pub prefix: String,
    pub fee: u128,
    pub denom: String,
}
