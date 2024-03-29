use anyhow::Context;
use cosmos_sdk_proto::cosmos::authz::v1beta1::{
    GenericAuthorization, Grant, MsgExec, MsgGrant, MsgRevoke,
};
use cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmos_sdk_proto::cosmos::distribution::v1beta1::{
    MsgWithdrawDelegatorReward, QueryDelegationTotalRewardsRequest,
    QueryDelegationTotalRewardsResponse,
};
use cosmos_sdk_proto::cosmos::feegrant::v1beta1::{
    BasicAllowance, MsgGrantAllowance, MsgRevokeAllowance,
};
use cosmos_sdk_proto::cosmos::gov::v1beta1::MsgVote;
use cosmos_sdk_proto::cosmos::staking::v1beta1::stake_authorization::{Policy, Validators};
use cosmos_sdk_proto::cosmos::staking::v1beta1::{
    AuthorizationType, MsgDelegate, StakeAuthorization,
};
use cosmos_sdk_proto::prost_wkt_types::{Any, MessageSerde};

use crate::query::perform_rpc_query;
use crate::Result;

fn after_one_year() -> Result<chrono::NaiveDateTime> {
    Ok(chrono::Utc::now()
        .date_naive()
        .succ_opt()
        .and_then(|x| x.and_hms_opt(0, 0, 0))
        .context("should be something")?
        + chrono::Duration::days(365))
}

fn generate_authz_msgs(granter: &str, grantee: &str, msg_types: &[&str]) -> Result<Vec<MsgGrant>> {
    msg_types
        .iter()
        .map(|msg_type| {
            Ok(MsgGrant {
                granter: granter.into(),
                grantee: grantee.into(),
                grant: Some(Grant {
                    authorization: Some(Any::try_pack(GenericAuthorization {
                        msg: msg_type.to_owned().into(),
                    })?),
                    expiration: Some(after_one_year()?.into()),
                }),
            })
        })
        .collect::<Result<_>>()
}

fn generate_authz_revoke_msgs(
    granter: &str,
    grantee: &str,
    msg_types: &[&str],
) -> Result<Vec<MsgRevoke>> {
    msg_types
        .iter()
        .map(|msg_type| {
            Ok(MsgRevoke {
                granter: granter.into(),
                grantee: grantee.into(),
                msg_type_url: msg_type.to_string(),
            })
        })
        .collect::<Result<_>>()
}

pub fn local_token_transfer(
    granter: &str,
    grantee: &str,
    amount: u128,
    denom: &str,
) -> Result<MsgSend> {
    Ok(MsgSend {
        from_address: granter.into(),
        to_address: grantee.into(),
        amount: vec![Coin {
            denom: denom.into(),
            amount: amount.to_string(),
        }],
    })
}

pub fn unit_transfer(granter: &str, grantee: &str, denom: &str) -> Result<MsgSend> {
    local_token_transfer(granter, grantee, 1, denom)
}

pub fn generate_feeallowance(granter: &str, grantee: &str) -> Result<MsgGrantAllowance> {
    Ok(MsgGrantAllowance {
        granter: granter.into(),
        grantee: grantee.into(),
        allowance: Some(Any::try_pack(BasicAllowance {
            spend_limit: vec![],
            expiration: Some(after_one_year()?.into()),
        })?),
    })
}

pub fn generate_revoke_feeallowance(granter: &str, grantee: &str) -> Result<MsgRevokeAllowance> {
    Ok(MsgRevokeAllowance {
        granter: granter.into(),
        grantee: grantee.into(),
    })
}

pub fn generate_grant_exec(grantee: &str, granted_msgs: &[Any]) -> Result<MsgExec> {
    Ok(MsgExec {
        grantee: grantee.into(),
        msgs: granted_msgs.to_vec(),
    })
}

pub async fn claim_all_reward(
    address: &str,
    rpc_endpoint: &str,
) -> Result<Vec<MsgWithdrawDelegatorReward>> {
    let query = QueryDelegationTotalRewardsRequest {
        delegator_address: address.into(),
    };

    let resp: QueryDelegationTotalRewardsResponse = perform_rpc_query(rpc_endpoint, query).await?;

    Ok(resp
        .rewards
        .into_iter()
        .map(|reward| {
            let validator = reward.validator_address;
            MsgWithdrawDelegatorReward {
                delegator_address: address.into(),
                validator_address: validator,
            }
        })
        .collect())
}

pub fn restake_app_auth(granter: &str, grantee: &str, validator: &str) -> Result<MsgGrant> {
    Ok(MsgGrant {
        granter: granter.into(),
        grantee: grantee.into(),
        grant: Some(Grant {
            authorization: Some(Any::try_pack(StakeAuthorization {
                max_tokens: None,
                authorization_type: AuthorizationType::Delegate.into(),
                validators: Some(Policy::AllowList(Validators {
                    address: vec![validator.into()],
                })),
            })?),
            expiration: Some(after_one_year()?.into()),
        }),
    })
}

pub fn restake_app_auth_revoke(granter: &str, grantee: &str) -> Result<MsgRevoke> {
    Ok(MsgRevoke {
        granter: granter.into(),
        grantee: grantee.into(),
        msg_type_url: MsgDelegate::default().type_url().to_string(),
    })
}

pub fn generate_usual_auth(granter: &str, grantee: &str) -> Result<Vec<MsgGrant>> {
    generate_authz_msgs(
        granter,
        grantee,
        &[
            MsgWithdrawDelegatorReward::default().type_url(),
            MsgDelegate::default().type_url(),
            MsgVote::default().type_url(),
        ],
    )
}

pub fn generate_usual_revoke(granter: &str, grantee: &str) -> Result<Vec<MsgRevoke>> {
    generate_authz_revoke_msgs(
        granter,
        grantee,
        &[
            MsgWithdrawDelegatorReward::default().type_url(),
            MsgDelegate::default().type_url(),
            MsgVote::default().type_url(),
        ],
    )
}

pub fn delegate_to(amount: u128, denom: &str, validator: &str, delegator: &str) -> MsgDelegate {
    MsgDelegate {
        delegator_address: delegator.into(),
        validator_address: validator.into(),
        amount: Some(Coin {
            amount: amount.to_string(),
            denom: denom.into(),
        }),
    }
}
