use std::collections::HashMap;

use crate::{
    account::Account, endpoint::get_rpc_endpoints, msg::generate_grant_exec,
    utils::read_data_from_yaml, Result,
};
use anyhow::Context;
use clap::Subcommand;
use cosmos_sdk_proto::prost_wkt_types::Any;
use cosmos_sdk_proto::{
    cosmos::{
        base::v1beta1::Coin,
        gov::v1beta1::MsgVote,
        staking::v1beta1::{MsgBeginRedelegate, MsgDelegate},
    },
    cosmwasm::wasm::v1::MsgExecuteContract,
    ibc::applications::transfer::v1::MsgTransfer,
};
use futures::StreamExt;

use super::utils::{custom_coin, custom_io_string, VotePair};

#[derive(Subcommand, Debug)]
pub enum Transaction {
    Send {
        source: String,
        target: String,
        amount: u128,
    },
    Restake {
        account: String,
    },
    Delegate {
        account: String,
        amount: u128,
        validator: Option<String>,
    },
    Redelegate {
        account: String,
        source: String,
        target: String,
        amount: Option<u128>,
    },
    RestakeApp {
        granter: String,
        grantee: String,
        validator: String,
    },
    RestakeAppRevoke {
        granter: String,
        grantee: String,
    },
    Grant {
        granter: String,
        grantee: String,
    },
    Revoke {
        granter: String,
        grantee: String,
    },
    Vote {
        voter: String,
        #[clap(required = true)]
        votes: Vec<VotePair>,
    },
    IBCTransfer {
        source_channel: String,
        #[clap(value_parser(custom_coin))]
        token: Coin,
        sender: String,
        receiver_address: String,
    },
    Cosmwasm {
        #[clap(value_parser(custom_coin))]
        funds: Coin,
        sender: String,
        contract_address: String,
        #[clap(value_parser(custom_io_string))]
        json: String,
    },
}

impl Transaction {
    pub async fn run(
        &self,
        dry_run: bool,
        chain_id: &str,
        executor: Option<&str>,
        rpc: Option<&str>,
        fee: Option<&Coin>,
    ) -> crate::Result<()> {
        let project_dir =
            directories::ProjectDirs::from("systems", "rnbguy", "rover").context("project dir")?;
        let data_local_dir = project_dir.data_local_dir();
        std::fs::create_dir_all(data_local_dir)?;
        let accounts_path = data_local_dir.join("accounts.yaml");
        let accounts_path_str = accounts_path.to_str().context("project path")?;

        let accounts: HashMap<String, Account> = read_data_from_yaml(accounts_path_str)?;

        let chains_path = data_local_dir.join("chains.yaml");
        let chains_path_str = chains_path.to_str().context("project path")?;

        let chains: HashMap<String, crate::chain::Chain> = read_data_from_yaml(chains_path_str)?;

        let config_dir = project_dir.config_dir();
        let config_path = config_dir.join("config.yaml");
        let config_path_str = config_path.to_str().context("project path")?;

        println!("{:?}", accounts_path_str);
        println!("{:?}", accounts);

        let config: HashMap<String, String> = read_data_from_yaml(config_path_str)?;
        let graphql_endpoint = config.get("graphql").expect("not exists");

        let chain = chains.get(chain_id).expect("no chain?");

        let hrp = chain.prefix.as_str();
        let fee = match fee {
            Some(Coin { denom, amount }) => (amount.parse::<u128>()?, denom.as_str()),
            None => (chain.fee, chain.denom.as_str()),
        };
        let denom = chain.denom.as_str();
        let fee_granter = String::new();
        let mut rpc_endpoints = get_rpc_endpoints(chain_id, graphql_endpoint).await?;

        rpc_endpoints = rpc_endpoints.into_iter().skip(3).collect();

        if let Some(rpc_endpoint) = rpc {
            rpc_endpoints.push((0, rpc_endpoint.into()))
        }

        let (owner, account_number, unsigned_tx) = futures::stream::iter(rpc_endpoints.iter())
            .then(|(_, rpc_endpoint)| {
                let accounts = accounts.clone();
                let mut fee_granter = fee_granter.clone();
                async move {
                    println!("trying with {}", &rpc_endpoint);
                    let (mut owner, mut any_msgs) = match &self {
                        Self::Send {
                            source: source_key,
                            target: target_key,
                            amount,
                        } => {
                            let mut any_msgs = vec![];

                            let source_acc = accounts.get(source_key).expect("not exists");
                            let target_acc = accounts.get(target_key).expect("not exists");

                            let source = source_acc.address(hrp)?;
                            let target = target_acc.address(hrp)?;

                            let local_transfer =
                                crate::msg::local_token_transfer(&source, &target, *amount, denom)?;

                            any_msgs.push(Any::try_pack(local_transfer)?);

                            (source_acc, any_msgs)
                        }
                        Self::Restake {
                            account: account_key,
                        } => {
                            let account_acc = accounts.get(account_key).expect("not exists");
                            let account = account_acc.address(hrp)?;

                            let delegations =
                                crate::query::get_delegated(&account, rpc_endpoint).await?;
                            let rewards = crate::query::get_rewards(&account, rpc_endpoint).await?;

                            let validator = delegations
                                .get(0)
                                .context("delegator should have atleast one validator")?
                                .0
                                .clone();

                            let total_delegations =
                                delegations.into_iter().map(|x| x.1).sum::<u128>();

                            let total_rewards = rewards
                                .into_iter()
                                .find(|x| x.0 == denom)
                                .map(|x| x.1)
                                .unwrap_or(0);

                            let mut any_msgs = vec![];

                            if (total_rewards * total_rewards) > (fee.0 * total_delegations) {
                                let withdraw_msgs =
                                    crate::msg::claim_all_reward(&account, rpc_endpoint).await?;

                                any_msgs.extend(
                                    withdraw_msgs
                                        .into_iter()
                                        .map(|x| Ok(Any::try_pack(x)?))
                                        .collect::<Result<Vec<_>>>()?,
                                );
                                const BUFFER_BALANCE_AMOUNT: u128 = 10_000;

                                if total_rewards > BUFFER_BALANCE_AMOUNT {
                                    let delegate_msg = crate::msg::delegate_to(
                                        total_rewards,
                                        denom,
                                        &validator,
                                        &account,
                                    );

                                    any_msgs.push(Any::try_pack(delegate_msg)?);
                                }
                            }
                            (account_acc, any_msgs)
                        }
                        Self::Delegate {
                            account: account_key,
                            validator: validator_opt,
                            amount,
                        } => {
                            let account_acc = accounts.get(account_key).expect("not exists");
                            let account = account_acc.address(hrp)?;
                            let mut any_msgs = vec![];

                            let validator = match validator_opt {
                                Some(validator) => validator.into(),
                                None => {
                                    let delegations =
                                        crate::query::get_delegated(&account, rpc_endpoint).await?;

                                    delegations
                                        .get(0)
                                        .context("delegator should have atleast one validator")?
                                        .0
                                        .clone()
                                }
                            };

                            let delegate_msg = MsgDelegate {
                                delegator_address: account,
                                validator_address: validator,
                                amount: Some(Coin {
                                    denom: denom.into(),
                                    amount: amount.to_string(),
                                }),
                            };

                            any_msgs.push(Any::try_pack(delegate_msg)?);

                            (account_acc, any_msgs)
                        }
                        Self::Redelegate {
                            account: account_key,
                            source,
                            target,
                            amount,
                        } => {
                            let account_acc = accounts.get(account_key).expect("not exists");
                            let account = account_acc.address(hrp)?;
                            let mut any_msgs = vec![];

                            let final_amount = match amount {
                                Some(value) => *value,
                                None => {
                                    let m: HashMap<_, _> =
                                        crate::query::get_delegated(&account, rpc_endpoint)
                                            .await?
                                            .into_iter()
                                            .collect();
                                    m[source]
                                }
                            };

                            let redelegate_msg = MsgBeginRedelegate {
                                delegator_address: account,
                                validator_src_address: source.into(),
                                validator_dst_address: target.into(),
                                amount: Some(Coin {
                                    denom: denom.into(),
                                    amount: final_amount.to_string(),
                                }),
                            };

                            any_msgs.push(Any::try_pack(redelegate_msg)?);

                            (account_acc, any_msgs)
                        }
                        Self::RestakeApp {
                            granter: granter_key,
                            grantee,
                            validator,
                        } => {
                            let granter_acc = accounts.get(granter_key).expect("not exists");
                            let granter = granter_acc.address(hrp)?;

                            let restake_app_msg =
                                crate::msg::restake_app_auth(&granter, grantee, validator)?;

                            (granter_acc, vec![Any::try_pack(restake_app_msg)?])
                        }
                        Self::RestakeAppRevoke {
                            granter: granter_key,
                            grantee,
                        } => {
                            let granter_acc = accounts.get(granter_key).expect("not exists");
                            let granter = granter_acc.address(hrp)?;

                            let restake_app_msg =
                                crate::msg::restake_app_auth_revoke(&granter, grantee)?;

                            (granter_acc, vec![Any::try_pack(restake_app_msg)?])
                        }
                        Self::Grant {
                            granter: granter_key,
                            grantee: grantee_key,
                        } => {
                            let granter_acc = accounts.get(granter_key).expect("not exists");
                            let grantee_acc = accounts.get(grantee_key).expect("not exists");

                            let granter = granter_acc.address(hrp)?;
                            let grantee = grantee_acc.address(hrp)?;

                            let mut any_msgs = vec![];

                            let authz_msgs = crate::msg::generate_usual_auth(&granter, &grantee)?;

                            any_msgs.extend(
                                authz_msgs
                                    .into_iter()
                                    .map(|x| Ok(Any::try_pack(x)?))
                                    .collect::<Result<Vec<_>>>()?,
                            );

                            let fee_allowance =
                                crate::msg::generate_feeallowance(&granter, &grantee)?;

                            any_msgs.push(Any::try_pack(fee_allowance)?);

                            let unit = crate::msg::unit_transfer(&granter, &grantee, denom)?;

                            any_msgs.push(Any::try_pack(unit)?);

                            (granter_acc, any_msgs)
                        }
                        Self::Revoke {
                            granter: granter_key,
                            grantee: grantee_key,
                        } => {
                            let granter_acc = accounts.get(granter_key).expect("not exists");
                            let grantee_acc = accounts.get(grantee_key).expect("not exists");

                            let granter = granter_acc.address(hrp)?;
                            let grantee = grantee_acc.address(hrp)?;

                            let mut any_msgs = vec![];

                            let revoke_msgs =
                                crate::msg::generate_usual_revoke(&granter, &grantee)?;

                            any_msgs.extend(
                                revoke_msgs
                                    .into_iter()
                                    .map(|x| Ok(Any::try_pack(x)?))
                                    .collect::<Result<Vec<_>>>()?,
                            );

                            let fee_revoke_allowance =
                                crate::msg::generate_revoke_feeallowance(&granter, &grantee)?;

                            any_msgs.push(Any::try_pack(fee_revoke_allowance)?);

                            (granter_acc, any_msgs)
                        }
                        Self::Vote { voter, votes } => (
                            accounts.get(voter).expect("not exists"),
                            votes
                                .iter()
                                .map(|vote| {
                                    Ok(Any::try_pack(MsgVote {
                                        proposal_id: vote.proposal_id,
                                        voter: accounts
                                            .get(voter)
                                            .context("voter is not in accounts")?
                                            .address(hrp)?,
                                        option: vote.option.into(),
                                    })?)
                                })
                                .collect::<Result<Vec<_>>>()?,
                        ),
                        Self::IBCTransfer {
                            source_channel,
                            token,
                            sender,
                            receiver_address,
                        } => {
                            let account_acc = accounts.get(sender).expect("not exists");
                            let account = account_acc.address(hrp)?;

                            let ibc_transfer = MsgTransfer {
                                source_port: "transfer".into(),
                                source_channel: source_channel.into(),
                                token: Some(token.clone()),
                                sender: account,
                                receiver: receiver_address.into(),
                                timeout_height: None,
                                timeout_timestamp: (chrono::Utc::now()
                                    + chrono::Duration::minutes(10))
                                .timestamp_nanos()
                                    as u64,
                            };
                            (account_acc, vec![Any::try_pack(ibc_transfer)?])
                        }
                        Self::Cosmwasm {
                            json,
                            funds,
                            sender,
                            contract_address,
                        } => {
                            let account_acc = accounts.get(sender).expect("not exists");
                            let account = account_acc.address(hrp)?;

                            let cw_execute = MsgExecuteContract {
                                sender: account,
                                contract: contract_address.into(),
                                msg: json.as_bytes().to_vec(),
                                funds: vec![funds.clone()],
                            };

                            (account_acc, vec![Any::try_pack(cw_execute)?])
                        }
                    };

                    if let Some(grantee) = executor {
                        fee_granter = owner.address(hrp)?;
                        owner = accounts.get(grantee).expect("not exists");
                        any_msgs = vec![Any::try_pack(generate_grant_exec(
                            &owner.address(hrp)?,
                            &any_msgs,
                        )?)?];
                    }

                    let (acc_number, tx) = owner
                        .generate_unsigned_transaction(
                            &owner.address(hrp)?,
                            &any_msgs,
                            fee,
                            &fee_granter,
                            rpc_endpoint,
                        )
                        .await?;

                    Result::Ok((owner.clone(), acc_number, tx))
                }
            })
            .filter_map(|x| async { x.ok() })
            .boxed_local()
            .next()
            .await
            .expect("not able to create tx");

        println!("{}", serde_json::to_string_pretty(&unsigned_tx)?);

        let signed_tx = futures::stream::iter(rpc_endpoints.iter())
            .then(|(_, rpc_endpoint)| {
                let unsigned_tx = unsigned_tx.clone();
                let owner = owner.clone();
                async move {
                    println!("add gas and trying with {}", &rpc_endpoint);

                    let signed_but_needed_gas_tx = owner
                        .sign_unsigned_transaction(&unsigned_tx, chain_id, account_number)
                        .await?;

                    let needed_gas = crate::broadcast::simulate_via_tendermint_rpc(
                        rpc_endpoint,
                        signed_but_needed_gas_tx.clone(),
                    )
                    .await?;

                    let unsigned_tx = crate::txs::update_tx_with_gas(
                        unsigned_tx,
                        needed_gas + (needed_gas >> 2),
                    )?;

                    let signed_tx = owner
                        .sign_unsigned_transaction(&unsigned_tx, chain_id, account_number)
                        .await?;

                    println!("{}", serde_json::to_string_pretty(&signed_tx)?);

                    Result::Ok(signed_tx)
                }
            })
            .filter_map(|x| async { x.ok() })
            .boxed_local()
            .next()
            .await
            .expect("not able to sign and add gas fee tx");

        println!("{}", serde_json::to_string_pretty(&signed_tx)?);

        if !dry_run {
            let _rpc_result = futures::stream::iter(rpc_endpoints.iter())
                .then(|(_, rpc_endpoint)| {
                    let signed_tx = signed_tx.clone();
                    async move {
                        println!("broadcasting trying with {}", &rpc_endpoint);
                        let resp = tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            crate::broadcast::broadcast_via_tendermint_rpc(
                                rpc_endpoint,
                                &signed_tx,
                            ),
                        )
                        .await??;

                        println!("{:?}", resp);

                        (resp.code.is_ok())
                            .then_some(rpc_endpoint)
                            .context("this endpoint does not work")?;
                        Result::Ok(())
                    }
                })
                .filter_map(|x| async { x.ok() })
                .boxed_local()
                .next()
                .await;
            // if rpc_result.is_none() {
            //     futures::stream::iter(["https://api-meme-1.meme.sx"])
            //         .then(|rest_endpoint| {
            //             let signed_tx = signed_tx.clone();
            //             async move {
            //                 println!("broadcasting trying with rest {}", &rest_endpoint);
            //                 let resp = tokio::time::timeout(
            //                     std::time::Duration::from_secs(5),
            //                     crate::broadcast::broadcast_via_rest(rest_endpoint, &signed_tx),
            //                 )
            //                 .await??;

            //                 println!("{:?}", resp);

            //                 (resp.code == 0).then(|| rest_endpoint).context(
            //                     "this endpoint does not work"
            //                 )?;
            //                 Result::Ok(())
            //             }
            //         })
            //         .filter_map(|x| async { x.ok() })
            //         .boxed_local()
            //         .next()
            //         .await
            //         .expect("not able to broadcast");
            // }
        }

        Ok(())
    }
}
