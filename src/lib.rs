// https://iancoleman.io/bip39

pub mod broadcast;
pub mod data;
pub mod endpoint;
pub mod error;
pub mod keys;
pub mod ledger;
pub mod msg;
pub mod query;
pub mod txs;
pub mod utils;

pub type Result<O> = anyhow::Result<O>;

pub const CHAIN_DATA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/chain.yaml");
