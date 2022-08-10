use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use bip32::{secp256k1::ecdsa::SigningKey, DerivationPath, Language, Mnemonic, XPrv};

use std::collections::HashMap;

use crate::Result;

use bip32::PublicKey;
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

use bech32::{ToBase32, Variant};

// https://iancoleman.io/bip39

lazy_static::lazy_static! {
    /// This is an example for using doc comment attributes
    static ref MEMORY_KEYRING: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::new(Mutex::new(HashMap::new()));
}

pub fn key_from_mnemonic(mmseed: &str, coin: u64) -> Result<XPrv> {
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

pub fn get_priv_key_from_os(key_name: &str) -> Result<SigningKey> {
    let entry = keyring::Entry::new("rover", key_name);
    let priv_bytes = base64::decode(entry.get_password()?)?;
    Ok(SigningKey::from_bytes(&priv_bytes).expect("error"))
}

pub fn save_key_to_memory_from_mmseed(mmseed: &str, name: &str, coin: u64) -> Result<()> {
    let priv_key = crate::keys::key_from_mnemonic(mmseed, coin)?;
    save_key_to_memory(&priv_key.to_bytes(), name)?;
    Ok(())
}

pub fn save_key_to_memory(bytes: &[u8], name: &str) -> Result<()> {
    let mut keyring = MEMORY_KEYRING.lock().expect("Error");
    keyring.insert(name.into(), bytes.to_vec());
    Ok(())
}

pub fn get_priv_key_from_memory(key_name: &str) -> Result<SigningKey> {
    let keyring = MEMORY_KEYRING.lock().expect("Error");
    let priv_bytes = keyring
        .get(key_name)
        .context("key is not present in memory")?;
    Ok(SigningKey::from_bytes(priv_bytes).expect("error"))
}

pub fn from_pk_to_bech32_address<K>(pub_key: &K, prefix: &str) -> Result<String>
where
    K: PublicKey,
{
    let pk_hash = {
        let mut hasher = Sha256::new();
        hasher.update(pub_key.to_bytes());
        hasher.finalize()
    };

    let rip_result = {
        let mut rip_hasher = Ripemd160::new();
        rip_hasher.update(pk_hash);
        rip_hasher.finalize()
    };

    Ok(bech32::encode(
        prefix,
        rip_result.to_vec().to_base32(),
        Variant::Bech32,
    )?)
}

pub fn save_key_to_os_from_mmseed(mmseed: &str, keyname: &str, coin: u64) -> Result<()> {
    let priv_key = crate::keys::key_from_mnemonic(mmseed, coin)?;
    save_key_to_os(&priv_key.to_bytes(), keyname)?;
    println!("{}", base64::encode(priv_key.public_key().to_bytes()));
    Ok(())
}

pub fn save_key_to_os(bytes: &[u8], keyname: &str) -> Result<()> {
    let entry = keyring::Entry::new("rover", keyname);
    let password = base64::encode(bytes);
    entry.set_password(&password)?;
    Ok(())
}
