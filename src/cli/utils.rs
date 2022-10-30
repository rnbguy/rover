use std::str::FromStr;

use anyhow::Context;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmos_sdk_proto::cosmos::gov::v1beta1::VoteOption;

use crate::account::KeyStoreBackend;
use crate::Result;
use std::io::BufRead;

pub fn custom_keystorebackend(backend_str: &str) -> Result<KeyStoreBackend> {
    Ok(if backend_str == "Ledger" {
        KeyStoreBackend::Ledger
    } else {
        backend_str
            .split_once(':')
            .and_then(|(t, k)| match t {
                "Os" => Some(KeyStoreBackend::Os(k.into())),
                "Memory" => Some(KeyStoreBackend::Memory(k.into())),
                _ => None,
            })
            .context("invalid memory")?
    })
}

#[derive(Debug, Clone)]
pub struct VotePair {
    pub proposal_id: u64,
    pub option: VoteOption,
}

impl FromStr for VotePair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (left, right) = s.split_once(':').context("error spliting into a pair")?;
        Ok(VotePair {
            proposal_id: left.parse()?,
            option: serde_json::from_str(&format!("\"{}\"", right))?,
        })
    }
}

pub fn custom_coin(coin_str: &str) -> Result<Coin> {
    let amount = coin_str
        .chars()
        .take_while(|x| x.is_numeric())
        .collect::<String>();
    let denom = coin_str
        .chars()
        .skip_while(|x| x.is_numeric())
        .collect::<String>();
    Ok(Coin { denom, amount })
}

pub fn custom_io_string(json_str: &str) -> Result<String> {
    Ok(match json_str {
        "-" => std::io::stdin().lock().lines().flatten().collect(),
        _ if json_str.starts_with('@') => {
            std::fs::read_to_string(json_str.strip_prefix('@').context("should never arise")?)?
        }
        _ => json_str.into(),
    })
}
