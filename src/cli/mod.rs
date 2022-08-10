use std::collections::HashMap;

use anyhow::Context;
use clap::Parser;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;

use crate::{
    account::{Account, KeyStoreBackend},
    endpoint::{get_rpc_endpoints, transform_to_grpc_endpoint},
    keys::{save_key_to_os_from_mmseed, AddressType},
    query::{get_chain_id_info, get_chain_id_rpc, get_rpc_endpoint_chain_info},
    utils::{read_data_from_yaml, write_data_as_yaml},
    Result,
};

pub mod tx;
pub mod utils;

use utils::{custom_coin, custom_keystorebackend};

#[derive(Parser, Debug)]
pub enum Args {
    Endpoint {
        chain_id: String,
        #[clap(short, long)]
        grpc: bool,
    },
    RefreshEndpoint {
        chain_id: String,
    },
    Tx {
        chain_id: String,
        executor: Option<String>,
        #[clap(long, short)]
        rpc: Option<String>,
        #[clap(long, short, parse(try_from_str = custom_coin))]
        fee: Option<Coin>,
        #[clap(subcommand)]
        transaction: tx::Transaction,
    },
    AddAccount {
        #[clap(parse(try_from_str = custom_keystorebackend))]
        keystore: KeyStoreBackend,
        key: String,
        #[clap(value_enum)]
        addr_type: AddressType,
    },
    AddChain {
        chain_id: String,
        prefix: String,
        fee: u128,
        denom: String,
    },
    AddChainIdInfo {
        chain_id: String,
    },
    AddRPCInfo {
        endpoint: String,
    },
    ListZonesFromMapOfZones,
    AddChainFromEmeris,
    AddChainFromCosmosDirectory,
    AddChainFromPingPub,
    AddKeyToOs {
        key: String,
        #[clap(default_value_t = 118)]
        coin_type: u64,
    },
    Config {
        key: String,
        value: Option<String>,
    },
}

impl Args {
    pub async fn run(&self) -> crate::Result<()> {
        #[cfg(feature = "obfstr")]
        {
            let memory_key_name = obfstr::obfstr!("mem_key").to_string();
            {
                // PRIV_KEY=$(secret-tool lookup service rover username laptop-key application rust-keyring) cargo run -- ...
                let memory_base64 = obfstr::obfstr!(env!("PRIV_KEY")).to_string();
                // let memory_base64 = obfstr::obfstr!("PRIVKEY").to_string();
                let secret_bytes =
                    base64::decode(memory_base64).expect("error while base64 decode");
                crate::keys::save_key_to_memory(&secret_bytes, &memory_key_name)
                    .expect("Error while storing");
            }
        }
        match &self {
            Self::Endpoint { chain_id, grpc } => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;

                let config_dir = project_dir.config_dir();
                let config_path = config_dir.join("config.yaml");
                let config_path_str = config_path.to_str().context("project path")?;

                let config: HashMap<String, String> =
                    read_data_from_yaml(config_path_str).unwrap_or_default();
                let graphql_endpoint = config.get("graphql").expect("not exists");

                let endpoints = if *grpc {
                    futures::future::join_all(
                        get_rpc_endpoints(chain_id, graphql_endpoint)
                            .await?
                            .into_iter()
                            .map(|(height, endpoint)| async move {
                                Result::Ok((height, transform_to_grpc_endpoint(&endpoint).await?))
                            }),
                    )
                    .await
                    .into_iter()
                    .flatten()
                    .collect()
                } else {
                    get_rpc_endpoints(chain_id, graphql_endpoint).await?
                };

                for (height, endpoint) in endpoints {
                    println!("{} : {}", height, endpoint);
                }
                Ok(())
            }
            Self::RefreshEndpoint { .. } => todo!(),
            Self::Tx {
                chain_id,
                executor,
                transaction,
                rpc,
                fee,
            } => {
                transaction
                    .run(chain_id, executor.as_deref(), rpc.as_deref(), fee.as_ref())
                    .await
            }
            Self::AddAccount {
                keystore,
                key,
                addr_type,
            } => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;
                let accounts_path = data_local_dir.join("accounts.yaml");
                let accounts_path_str = accounts_path.to_str().context("project path")?;
                let mut accounts: HashMap<String, Account> =
                    read_data_from_yaml(accounts_path_str).unwrap_or_default();
                let new_account = Account::new(keystore.clone(), addr_type).await?;
                accounts.insert(key.into(), new_account);
                write_data_as_yaml(accounts_path_str, accounts)?;
                println!("Added to {}", accounts_path_str);
                Ok(())
            }
            Self::AddKeyToOs { key, coin_type } => {
                let mmseed = rpassword::prompt_password("Mnemonic 🔑: ").unwrap();
                save_key_to_os_from_mmseed(mmseed.trim(), key, *coin_type)?;
                Ok(())
            }
            Self::Config { key, value } => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let config_dir = project_dir.config_dir();
                std::fs::create_dir_all(config_dir)?;
                let config_path = config_dir.join("config.yaml");
                let config_path_str = config_path.to_str().context("project path")?;
                let mut config: HashMap<String, String> =
                    read_data_from_yaml(config_path_str).unwrap_or_default();
                if let Some(value) = value {
                    config.insert(key.into(), value.into());
                    write_data_as_yaml(config_path_str, config)?;
                } else {
                    println!("{} : {:?}", key, config.get(key));
                }
                Ok(())
            }
            Self::AddChain {
                chain_id,
                prefix,
                fee,
                denom,
            } => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;
                let chains_path = data_local_dir.join("chains.yaml");
                let chains_path_str = chains_path.to_str().context("project path")?;

                let mut chains: HashMap<String, crate::chain::Chain> =
                    read_data_from_yaml(chains_path_str).unwrap_or_default();

                chains.insert(
                    chain_id.into(),
                    crate::chain::Chain {
                        chain_id: chain_id.into(),
                        prefix: prefix.into(),
                        fee: *fee,
                        denom: denom.into(),
                    },
                );
                write_data_as_yaml(chains_path_str, chains)?;

                Ok(())
            }

            Self::AddChainIdInfo { chain_id } => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;

                let config_dir = project_dir.config_dir();
                let config_path = config_dir.join("config.yaml");
                let config_path_str = config_path.to_str().context("project path")?;

                let config: HashMap<String, String> =
                    read_data_from_yaml(config_path_str).unwrap_or_default();
                let graphql_endpoint = config.get("graphql").expect("not exists");

                let chain_info = get_chain_id_info(chain_id, graphql_endpoint).await?;

                println!("{}", serde_json::to_string_pretty(&chain_info)?);

                let chains_path = data_local_dir.join("chains.yaml");
                let chains_path_str = chains_path.to_str().context("project path")?;

                let mut chains: HashMap<String, crate::chain::Chain> =
                    read_data_from_yaml(chains_path_str).unwrap_or_default();

                chains.insert(chain_id.into(), chain_info);
                write_data_as_yaml(chains_path_str, chains)?;

                Ok(())
            }

            Self::AddRPCInfo { endpoint } => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;

                let chain_info = get_rpc_endpoint_chain_info(endpoint).await?;

                println!("{}", serde_json::to_string_pretty(&chain_info)?);

                let chains_path = data_local_dir.join("chains.yaml");
                let chains_path_str = chains_path.to_str().context("project path")?;

                let mut chains: HashMap<String, crate::chain::Chain> =
                    read_data_from_yaml(chains_path_str).unwrap_or_default();

                chains.insert(chain_info.chain_id.clone(), chain_info);
                write_data_as_yaml(chains_path_str, chains)?;

                Ok(())
            }

            Self::ListZonesFromMapOfZones => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;

                let config_dir = project_dir.config_dir();
                let config_path = config_dir.join("config.yaml");
                let config_path_str = config_path.to_str().context("project path")?;

                let config: HashMap<String, String> =
                    read_data_from_yaml(config_path_str).unwrap_or_default();
                let graphql_endpoint = config.get("graphql").expect("not exists");

                let zones = crate::endpoint::get_zone_ids(graphql_endpoint).await?;

                zones.into_iter().for_each(|zone| println!("{zone}"));

                Ok(())
            }

            Self::AddChainFromEmeris => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;
                let chains_path = data_local_dir.join("chains.yaml");
                let chains_path_str = chains_path.to_str().context("project path")?;

                let mut chains: HashMap<String, crate::chain::Chain> =
                    read_data_from_yaml(chains_path_str).unwrap_or_default();
                let emeris_chains: serde_json::Value =
                    ureq::get("https://api.emeris.com/v1/chains")
                        .call()?
                        .into_json()?;

                for emeris_chain in emeris_chains
                    .pointer("/chains")
                    .unwrap()
                    .as_array()
                    .unwrap()
                {
                    let chain_name = emeris_chain
                        .pointer("/chain_name")
                        .unwrap()
                        .as_str()
                        .unwrap();

                    let data: serde_json::Value =
                        ureq::get(&format!("https://api.emeris.com/v1/chain/{chain_name}"))
                            .call()?
                            .into_json()?;

                    let chain_id = data
                        .pointer("/chain/node_info/chain_id")
                        .unwrap()
                        .as_str()
                        .unwrap();
                    let prefix = data
                        .pointer("/chain/node_info/bech32_config/prefix_account")
                        .unwrap()
                        .as_str()
                        .unwrap();
                    let fee = &0;
                    let denom = data
                        .pointer("/chain/denoms")
                        .unwrap()
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|x| {
                            x.pointer("/fee_token")
                                .unwrap_or(&serde_json::Value::Bool(false))
                                .as_bool()
                                .unwrap()
                        })
                        .unwrap()
                        .pointer("/name")
                        .unwrap()
                        .as_str()
                        .unwrap();

                    chains.insert(
                        chain_id.into(),
                        crate::chain::Chain {
                            chain_id: chain_id.into(),
                            prefix: prefix.into(),
                            fee: *fee,
                            denom: denom.into(),
                        },
                    );
                }
                write_data_as_yaml(chains_path_str, &chains)?;
                Ok(())
            }

            Self::AddChainFromCosmosDirectory => {
                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;
                let chains_path = data_local_dir.join("chains.yaml");
                let chains_path_str = chains_path.to_str().context("project path")?;

                let mut chains: HashMap<String, crate::chain::Chain> =
                    read_data_from_yaml(chains_path_str).unwrap_or_default();
                let cd_chains: serde_json::Value = ureq::get("https://chains.cosmos.directory")
                    .call()?
                    .into_json()?;

                for cd_chain in cd_chains.pointer("/chains").unwrap().as_array().unwrap() {
                    let chain_name = cd_chain.pointer("/name").and_then(|x| x.as_str()).unwrap();

                    let data: serde_json::Value =
                        ureq::get(&format!("https://chains.cosmos.directory/{chain_name}"))
                            .call()?
                            .into_json()?;

                    let chain_id = data.pointer("/chain/chain_id").and_then(|x| x.as_str());
                    let prefix = data
                        .pointer("/chain/bech32_prefix")
                        .and_then(|x| x.as_str());
                    let fee = &0;
                    let denom = cd_chain.pointer("/denom").and_then(|x| x.as_str());

                    match (chain_id, prefix, denom) {
                        (Some(chain_id), Some(prefix), Some(denom)) => {
                            chains.insert(
                                chain_id.into(),
                                crate::chain::Chain {
                                    chain_id: chain_id.into(),
                                    prefix: prefix.into(),
                                    fee: *fee,
                                    denom: denom.into(),
                                },
                            );
                        }
                        _ => println!("{}", chain_name),
                    }
                }
                write_data_as_yaml(chains_path_str, &chains)?;
                Ok(())
            }

            Self::AddChainFromPingPub => {
                let main_page = ureq::get("https://ping.pub").call()?.into_string()?;
                let js_link_pattern = regex::Regex::new("/js/app.[^.]+.js")?;
                let js_link = js_link_pattern
                    .find(&main_page)
                    .expect("atleast one js link")
                    .as_str();
                let js_str = ureq::get(&format!("https://ping.pub{js_link}"))
                    .call()?
                    .into_string()?;
                let json_pattern = regex::Regex::new(r"JSON.parse\('([^']+)'\)")?;

                let pingpub_chains = json_pattern
                    .captures_iter(&js_str)
                    .map(|x| x.get(1).expect("atleast one json").as_str())
                    .flat_map(serde_json::from_str)
                    .collect::<Vec<serde_json::Value>>();

                let project_dir = directories::ProjectDirs::from("systems", "rnbguy", "rover")
                    .context("project dir")?;
                let data_local_dir = project_dir.data_local_dir();
                std::fs::create_dir_all(data_local_dir)?;
                let chains_path = data_local_dir.join("chains.yaml");
                let chains_path_str = chains_path.to_str().context("project path")?;

                let mut chains: HashMap<String, crate::chain::Chain> =
                    read_data_from_yaml(chains_path_str).unwrap_or_default();

                for chain in pingpub_chains {
                    println!("{}", &serde_json::to_string_pretty(&chain)?);
                    let chain_name = chain
                        .pointer("/chain_name")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default();
                    let prefix = chain
                        .pointer("/addr_prefix")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default();
                    let denom = chain
                        .pointer("/assets/0/base")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default();
                    let rpcs = chain
                        .pointer("/rpc")
                        .and_then(|x| {
                            x.as_array()
                                .map(|x| x.iter().map(|y| y.as_str().unwrap_or_default()).collect())
                                .or_else(|| x.as_str().map(|x| vec![x]))
                        })
                        .unwrap_or_default();
                    let chain_id = &futures::future::join_all(
                        rpcs.iter()
                            .map(|rpc_endpoint| async { get_chain_id_rpc(rpc_endpoint).await })
                            .collect::<Vec<_>>(),
                    )
                    .await
                    .into_iter()
                    .flatten()
                    .next()
                    .unwrap_or_else(|| {
                        let question = requestty::Question::input("first_name")
                            .message(&format!("Provide chain-id for {chain_name}"))
                            .build();
                        requestty::prompt_one(question)
                            .ok()
                            .and_then(|x| x.as_string().map(|x| x.into()))
                            .expect("answer")
                    });
                    let fee = &0;

                    if !chain_id.is_empty() && !prefix.is_empty() && !rpcs.is_empty() {
                        chains.insert(
                            chain_id.into(),
                            crate::chain::Chain {
                                chain_id: chain_id.into(),
                                prefix: prefix.into(),
                                fee: *fee,
                                denom: denom.into(),
                            },
                        );
                    } else {
                        println!("not storing");
                    }
                }

                write_data_as_yaml(chains_path_str, &chains)?;
                Ok(())
            }
        }
    }
}
