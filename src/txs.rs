use bip32::secp256k1::ecdsa::{signature::Signer, signature::Verifier, Signature};

use cosmos_sdk_proto::cosmos::tx::v1beta1::{AuthInfo, TxBody};
use cosmos_sdk_proto::cosmos::vesting::v1beta1::ContinuousVestingAccount;
use cosmos_sdk_proto::cosmos::{
    auth::v1beta1::BaseAccount,
    crypto::secp256k1::PubKey,
    tx::v1beta1::{Fee, ModeInfo, SignDoc, Tx},
};

use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;

use cosmos_sdk_proto::cosmos::tx::signing::v1beta1::SignMode;
use cosmos_sdk_proto::cosmos::tx::v1beta1::{
    mode_info::{Single, Sum},
    SignerInfo,
};

use cosmos_sdk_proto::prost_wkt_types::{Any, MessageSerde};
use serde_json::Value;

use crate::Result;

pub fn generate_auth_info(
    public_key: PubKey,
    sequence: u64,
    gas: u64,
    fee_amount: &u128,
    fee_denom: &str,
    fee_granter: &str,
    mode: SignMode,
) -> Result<AuthInfo> {
    let mode_info = ModeInfo {
        sum: Some(Sum::Single(Single { mode: mode.into() })),
    };

    let signer_info = SignerInfo {
        public_key: Some(Any::try_pack(public_key)?),
        mode_info: Some(mode_info),
        sequence,
    };

    let fees = if fee_amount > &0 {
        vec![Coin {
            denom: fee_denom.into(),
            amount: fee_amount.to_string(),
        }]
    } else {
        vec![]
    };

    let fee = Fee {
        gas_limit: gas,
        payer: "".into(),
        granter: if fees.is_empty() {
            String::new()
        } else {
            fee_granter.into()
        },
        amount: fees,
    };

    Ok(AuthInfo {
        signer_infos: vec![signer_info],
        fee: Some(fee),
    })
}

pub fn set_fee(fee: (u128, &str), tx: &Tx) -> Tx {
    let mut tx = tx.clone();
    tx.auth_info
        .as_mut()
        .unwrap()
        .fee
        .as_mut()
        .unwrap()
        .amount
        .push(Coin {
            denom: fee.1.to_string(),
            amount: fee.0.to_string(),
        });
    tx
}

pub fn generate_signature_from_sign_doc<K>(sign_doc: SignDoc, priv_key: &K) -> Result<[u8; 64]>
where
    K: Signer<Signature>,
{
    let signature: Signature = priv_key.try_sign(&sign_doc.try_encoded()?).expect("error");
    Ok(signature.as_ref().try_into()?)
}

pub fn update_signature(mut tx: Tx, signature: &[u8; 64]) -> Tx {
    tx.signatures = vec![signature.to_vec()];
    tx
}

pub fn generate_sign_doc(tx: &Tx, chain_id: &str, account_number: u64) -> Result<SignDoc> {
    Ok(SignDoc {
        body_bytes: tx.body.as_ref().unwrap().try_encoded()?,
        auth_info_bytes: tx.auth_info.as_ref().unwrap().try_encoded()?,
        chain_id: chain_id.into(),
        account_number,
    })
}

pub fn verify_transaction<K>(
    signed_tx: &Tx,
    chain_id: &str,
    account_number: u64,
    pub_key: &K,
) -> Result<()>
where
    K: Verifier<Signature>,
{
    use bip32::secp256k1::ecdsa::signature::Signature;
    let sign_doc = crate::txs::generate_sign_doc(signed_tx, chain_id, account_number)?;
    let signature = signed_tx.signatures[0].clone();
    pub_key
        .verify(
            &sign_doc.try_encoded()?,
            &Signature::from_bytes(&signature).expect("error"),
        )
        .expect("error");
    Ok(())
}

fn is_registered(msg_type: &str) -> bool {
    // RegisterLegacyAminoCodec
    // bank, staking, distribution, feegrant, authz, gov
    // TODO: add other msg paths
    [
        "/cosmos.bank.v1beta1.MsgSend",
        "/cosmos.staking.v1beta1.MsgDelegate",
        "/cosmos.staking.v1beta1.MsgBeginRedelegate",
        "/cosmos.distribution.v1beta1.MsgWithdrawDelegatorReward",
        "/cosmos.gov.v1beta1.MsgVote",
        "/ibc.applications.transfer.v1.MsgTransfer",
    ]
    .into_iter()
    .any(|x| x == msg_type)
}

fn amino_json(mut msg: Value) -> Value {
    let map = msg.as_object_mut().unwrap();

    let (_, msg_type) = map.remove_entry("@type").unwrap();
    let msg_type = msg_type.as_str().unwrap();

    remove_type(&mut msg);

    if is_registered(msg_type) {
        let split_msg_type = msg_type.rsplit_once('.').unwrap().1;
        let split_msg_type = match split_msg_type {
            "MsgWithdrawDelegatorReward" => "MsgWithdrawDelegationReward",
            _ => split_msg_type,
        };
        let msg_type = format!("cosmos-sdk/{}", split_msg_type);
        serde_json::json!({
            "type": msg_type,
            "value": msg,
        })
    } else {
        msg
    }
}

fn remove_type(msg: &mut Value) {
    match msg {
        Value::Array(ref mut v) => v.iter_mut().for_each(remove_type),
        Value::Object(ref mut obj) => {
            obj.remove("@type");
            obj.values_mut().for_each(remove_type);
            obj.iter_mut().for_each(|(k, v)| {
                if k.ends_with("_id") {
                    *v = Value::String(v.to_string())
                }
                if k.ends_with("_timestamp") {
                    *v = Value::String(v.to_string())
                }
                if v.is_null() {
                    *v = serde_json::json!({});
                }
            })
        }
        _ => {}
    }
}

pub fn generate_legacy_amino_json(
    tx: &Tx,
    chain_id: &str,
    account_number: u64,
) -> Result<serde_json::Value> {
    let sequence = tx.auth_info.as_ref().unwrap().signer_infos[0].sequence;
    let gas = tx
        .auth_info
        .as_ref()
        .unwrap()
        .fee
        .as_ref()
        .unwrap()
        .gas_limit;
    let fee_amount = &tx.auth_info.as_ref().unwrap().fee.as_ref().unwrap().amount;
    let msgs: Vec<_> = tx
        .body
        .as_ref()
        .unwrap()
        .messages
        .iter()
        .map(|x| serde_json::to_value(x).expect("error"))
        .map(amino_json)
        .collect();

    // https://github.com/cosmos/ledger-cosmos/blob/main/docs/TXSPEC.md
    // it does not mention, but the key-values must be string
    // `account_number: "23"` instead of `account_number: 23`

    let payload = serde_json::json!({
      "account_number": account_number.to_string(),
      "chain_id": chain_id.to_string(),
      "fee": {
        "amount": fee_amount,
        "gas": gas.to_string()
      },
      "memo": "",
      "msgs": msgs,
      "sequence": sequence.to_string()
    });

    println!("{}", serde_json::to_string_pretty(&payload)?);

    Ok(payload)
}

pub async fn get_account_number_and_sequence(
    rpc_endpoint: &str,
    address: &str,
) -> Result<(u64, u64, Option<PubKey>)> {
    let info = crate::query::get_account_info(rpc_endpoint, address).await?;

    let bacc = info
        .account
        .clone()
        .unwrap()
        .unpack_as(BaseAccount::default())
        .or_else(|_| {
            info.account
                .unwrap()
                .unpack_as(ContinuousVestingAccount::default())
                .map(|x| x.base_vesting_account.unwrap().base_account.unwrap())
        })?;

    Ok((
        bacc.account_number,
        bacc.sequence,
        bacc.pub_key
            .map(|x| x.unpack_as(PubKey::default()))
            .transpose()?,
    ))
}

pub fn update_tx_with_gas(mut tx: Tx, gas: u64) -> Tx {
    let auth_info = tx.auth_info.as_mut().unwrap();
    let fee = auth_info.fee.as_mut().unwrap();
    fee.gas_limit = gas;
    tx
}

pub fn create_transaction(body: TxBody, auth_info: AuthInfo) -> Tx {
    Tx {
        body: Some(body),
        auth_info: Some(auth_info),
        signatures: vec![],
    }
}
