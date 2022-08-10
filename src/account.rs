use cosmos_sdk_proto::cosmos::{
    crypto::secp256k1::PubKey,
    tx::{
        signing::v1beta1::SignMode,
        v1beta1::{Tx, TxBody},
    },
};
use cosmos_sdk_proto::prost_wkt_types::Any;
use std::str::FromStr;

use crate::{
    keys::{
        from_pk_bytes_to_address, get_priv_key_from_memory, get_priv_key_from_os,
        get_uncompressed_pub_key_from_memory, get_uncompressed_pub_key_from_os, AddressType,
    },
    ledger::{get_pub_key, get_signature},
    txs::{
        create_transaction, generate_auth_info, generate_legacy_amino_json,
        get_account_number_and_sequence,
    },
    Result,
};
use bip32::{secp256k1::ecdsa::VerifyingKey, DerivationPath};
use bip32::{
    secp256k1::ecdsa::{signature::Signer, Signature},
    PrivateKey,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum KeyStoreBackend {
    Ledger,
    Os(String),
    Memory(String),
}

impl KeyStoreBackend {
    pub async fn get_signature(&self, data: &[u8]) -> Result<[u8; 64]> {
        match &self {
            Self::Ledger => {
                let value: serde_json::Value = serde_json::from_slice(data)?;
                Ok(get_signature(value, &DerivationPath::from_str("m/44'/118'/0'/0/0")?).await?)
            }
            Self::Os(key) => {
                let priv_key = get_priv_key_from_os(key)?;
                let signature: Signature = priv_key.try_sign(data).unwrap();
                Ok(signature.as_ref().try_into()?)
            }
            Self::Memory(key) => {
                let priv_key = get_priv_key_from_memory(key)?;
                let signature: Signature = priv_key.try_sign(data).unwrap();
                Ok(signature.as_ref().try_into()?)
            }
        }
    }

    pub async fn public_key(&self) -> Result<VerifyingKey> {
        Ok(match &self {
            Self::Ledger => VerifyingKey::from_sec1_bytes(
                &get_pub_key(
                    "cosmos",
                    &DerivationPath::from_str("m/44'/118'/0'/0/0")?,
                    false,
                )
                .await?[..33],
            )
            .expect("bip32 error"),
            Self::Os(key) => get_priv_key_from_os(key)?.public_key(),
            Self::Memory(key) => get_priv_key_from_memory(key)?.public_key(),
        })
    }

    pub async fn uncompressed_public_key_bytes(&self) -> Result<Vec<u8>> {
        Ok(match &self {
            Self::Ledger => get_pub_key(
                "cosmos",
                &DerivationPath::from_str("m/44'/118'/0'/0/0")?,
                false,
            )
            .await?[1..]
                .to_vec(),
            Self::Os(key) => get_uncompressed_pub_key_from_os(key)?,
            Self::Memory(key) => get_uncompressed_pub_key_from_memory(key)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    cosmos_address: String,
    private_key_backend: KeyStoreBackend,
}

impl Account {
    pub async fn new(
        private_key_backend: KeyStoreBackend,
        addr_type: &AddressType,
    ) -> Result<Self> {
        let public_key = match addr_type {
            AddressType::Cosmos => private_key_backend.public_key().await?.to_bytes().to_vec(),
            AddressType::Ethereum => private_key_backend.uncompressed_public_key_bytes().await?,
        };
        let address = from_pk_bytes_to_address(&public_key, addr_type)?;
        Ok(Self {
            cosmos_address: address,
            private_key_backend,
        })
    }

    pub fn address(&self, prefix: &str) -> Result<String> {
        crate::utils::bech32(&self.cosmos_address, prefix)
    }

    pub async fn generate_unsigned_transaction(
        &self,
        address: &str,
        any_msgs: &[Any],
        fee: (u128, &str),
        fee_granter: &str,
        rpc_endpoint: &str,
    ) -> Result<(u64, Tx)> {
        let (fee_amount, fee_denom) = fee;
        let tx_body = TxBody {
            messages: any_msgs.to_vec(),
            memo: "".into(),
            ..Default::default()
        };

        let (account_number, sequence, mut public_key_on_chain) =
            get_account_number_and_sequence(rpc_endpoint, address).await?;

        if public_key_on_chain.is_none() {
            public_key_on_chain = Some(PubKey {
                key: self
                    .private_key_backend
                    .public_key()
                    .await?
                    .to_bytes()
                    .to_vec(),
            });
        }

        let public_key = public_key_on_chain.expect("never");

        let auth_info = generate_auth_info(
            public_key,
            sequence,
            400_000,
            &fee_amount,
            fee_denom,
            fee_granter,
            match self.private_key_backend {
                KeyStoreBackend::Ledger => SignMode::LegacyAminoJson,
                _ => SignMode::Direct,
            },
        )?;

        Ok((account_number, create_transaction(tx_body, auth_info)))
    }

    pub async fn sign_unsigned_transaction(
        &self,
        unsigned_tx: &Tx,
        chain_id: &str,
        account_number: u64,
    ) -> Result<Tx> {
        Ok(match &self.private_key_backend {
            KeyStoreBackend::Ledger => {
                let payload = generate_legacy_amino_json(unsigned_tx, chain_id, account_number)?;
                let signature = crate::ledger::get_signature(
                    payload,
                    &DerivationPath::from_str("m/44'/118'/0'/0/0")?,
                )
                .await?;
                crate::txs::update_signature(unsigned_tx.clone(), &signature)
            }
            KeyStoreBackend::Os(key) => {
                let priv_key = get_priv_key_from_os(key)?;
                let sign_doc =
                    crate::txs::generate_sign_doc(unsigned_tx, chain_id, account_number)?;
                let signature = crate::txs::generate_signature_from_sign_doc(sign_doc, &priv_key)?;
                crate::txs::update_signature(unsigned_tx.clone(), &signature)
            }
            KeyStoreBackend::Memory(key) => {
                let priv_key = get_priv_key_from_memory(key)?;
                let sign_doc =
                    crate::txs::generate_sign_doc(unsigned_tx, chain_id, account_number)?;
                let signature = crate::txs::generate_signature_from_sign_doc(sign_doc, &priv_key)?;
                crate::txs::update_signature(unsigned_tx.clone(), &signature)
            }
        })
    }
}
