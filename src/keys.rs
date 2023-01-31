use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use bip32::{secp256k1::ecdsa::SigningKey, DerivationPath, Language, Mnemonic, XPrv};
use clap::ValueEnum;
use secp256k1::{PublicKey, Secp256k1, SecretKey};

use std::collections::HashMap;

use crate::Result;

use base64::prelude::{Engine as _, BASE64_STANDARD};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use sha3::Keccak256;

use bech32::{ToBase32, Variant};

// https://iancoleman.io/bip39

lazy_static::lazy_static! {
    /// This is an example for using doc comment attributes
    static ref MEMORY_KEYRING: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::new(Mutex::new(HashMap::new()));
}

fn key_from_mnemonic(mmseed: &str, coin: u64) -> Result<XPrv> {
    // let mmseed = "tip purse since square taste soccer future hat orbit blame anchor oppose onion garlic taxi daring aisle slide buzz theory bronze explain refuse surface";
    // let address = "cosmos10uuc6zj564lwhuvlutwsmsa2ruc8qmj6x8kp6x";
    println!("'{mmseed}'");
    let mnemonic = Mnemonic::new(mmseed, Language::English)?;
    let drv_path = format!("m/44'/{coin}'/0'/0/0");
    println!("using {drv_path}..");
    let derivation_path = DerivationPath::from_str(&drv_path)?;
    let seed = mnemonic.to_seed("");

    Ok(XPrv::derive_from_path(seed, &derivation_path)?)
}

pub fn cosmos_key_derive(bytes: &[u8]) -> Vec<u8> {
    Ripemd160::digest(Sha256::digest(bytes))[..20].to_vec()
}

pub fn ethereum_key_derive(bytes: &[u8]) -> Vec<u8> {
    assert_eq!(bytes.len(), 64);
    Keccak256::digest(bytes)
        .into_iter()
        .rev()
        .take(20)
        .rev()
        .collect()
}

#[derive(Debug, Clone, ValueEnum)]
pub enum AddressType {
    Cosmos,
    Ethereum,
}

pub fn from_pk_bytes_to_address(pub_key: &[u8], addr_type: &AddressType) -> Result<String> {
    let data = match addr_type {
        AddressType::Cosmos => cosmos_key_derive(pub_key),
        AddressType::Ethereum => ethereum_key_derive(pub_key),
    };
    let address = bech32::encode("cosmos", data.to_base32(), Variant::Bech32)?;
    println!("Cosmos address: {}", &address);
    Ok(address)
}

fn save_key_to_os(bytes: &[u8], keyname: &str) -> Result<()> {
    let entry = keyring::Entry::new("rover", keyname);
    let password = BASE64_STANDARD.encode(bytes);
    entry.set_password(&password)?;
    Ok(())
}

pub fn save_key_to_os_from_mmseed(mmseed: &str, keyname: &str, coin: u64) -> Result<()> {
    let priv_key = crate::keys::key_from_mnemonic(mmseed, coin)?;
    save_key_to_os(&priv_key.to_bytes(), keyname)?;
    println!(
        "{}",
        BASE64_STANDARD.encode(priv_key.public_key().to_bytes())
    );
    Ok(())
}

pub fn get_priv_key_from_os(key_name: &str) -> Result<SigningKey> {
    let entry = keyring::Entry::new("rover", key_name);
    let priv_bytes = BASE64_STANDARD.decode(entry.get_password()?)?;
    Ok(SigningKey::from_bytes(&priv_bytes).expect("error"))
}

pub fn get_uncompressed_pub_key_from_os(key_name: &str) -> Result<Vec<u8>> {
    let entry = keyring::Entry::new("rover", key_name);
    let priv_bytes = BASE64_STANDARD.decode(entry.get_password()?)?;
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&priv_bytes).expect("32 bytes, within curve order");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    Ok(public_key.serialize_uncompressed()[1..].to_vec())
}

pub fn save_key_to_memory(bytes: &[u8], name: &str) -> Result<()> {
    let mut keyring = MEMORY_KEYRING.lock().expect("Error");
    keyring.insert(name.into(), bytes.to_vec());
    Ok(())
}

pub fn save_key_to_memory_from_mmseed(mmseed: &str, name: &str, coin: u64) -> Result<()> {
    let priv_key = crate::keys::key_from_mnemonic(mmseed, coin)?;
    save_key_to_memory(&priv_key.to_bytes(), name)?;
    Ok(())
}

pub fn get_priv_key_from_memory(key_name: &str) -> Result<SigningKey> {
    let keyring = MEMORY_KEYRING.lock().expect("Error");
    let priv_bytes = keyring
        .get(key_name)
        .context("key is not present in memory")?;
    Ok(SigningKey::from_bytes(priv_bytes).expect("error"))
}

pub fn get_uncompressed_pub_key_from_memory(key_name: &str) -> Result<Vec<u8>> {
    let entry = keyring::Entry::new("rover", key_name);
    let priv_bytes = BASE64_STANDARD.decode(entry.get_password()?)?;
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&priv_bytes).expect("32 bytes, within curve order");
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    Ok(public_key.serialize_uncompressed()[1..].to_vec())
}

pub fn mnemonic_to_cosmos_addr(mnemonic: &Mnemonic, hrp: &str, coin_type: &u64) -> Result<String> {
    let drv_path = format!("m/44'/{coin_type}'/0'/0/0");
    let derivation_path = DerivationPath::from_str(&drv_path)?;
    let seed = mnemonic.to_seed("");
    let priv_key = XPrv::derive_from_path(seed, &derivation_path)?;
    let pub_key = priv_key.public_key();
    Ok(bech32::encode(
        hrp,
        cosmos_key_derive(pub_key.to_bytes().as_ref()).to_base32(),
        Variant::Bech32,
    )?)
}
