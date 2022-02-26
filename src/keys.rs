use std::str::FromStr;
use std::sync::{Arc, Mutex};

use bip32::{secp256k1::ecdsa::SigningKey, DerivationPath, Language, Mnemonic, XPrv};

use std::collections::HashMap;

use crate::Result;

use crate::error::Error;

use bip32::{ExtendedPublicKey, PublicKey};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

use bech32::{ToBase32, Variant};

// https://iancoleman.io/bip39

lazy_static::lazy_static! {
    /// This is an example for using doc comment attributes
    static ref OS_KEYRING: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::new(Mutex::new(HashMap::new()));
}

pub fn key_from_mnemonic(mmseed: &str) -> Result<XPrv> {
    // let mmseed = "tip purse since square taste soccer future hat orbit blame anchor oppose onion garlic taxi daring aisle slide buzz theory bronze explain refuse surface";
    // let address = "cosmos10uuc6zj564lwhuvlutwsmsa2ruc8qmj6x8kp6x";

    let mnemonic = Mnemonic::new(mmseed, Language::English)?;
    let derivation_path = DerivationPath::from_str("m/44'/118'/0'/0/0")?;
    let seed = mnemonic.to_seed("");

    Ok(XPrv::derive_from_path(seed, &derivation_path)?)
}

pub fn save_key_to_os_from_mmseed(mmseed: &str, name: &str) -> Result<()> {
    let priv_key = crate::keys::key_from_mnemonic(mmseed)?;
    save_key_to_os(&priv_key.to_bytes(), name)?;
    Ok(())
}

pub fn save_key_to_os(bytes: &[u8], name: &str) -> Result<()> {
    let entry = keyring::Entry::new("rover", name);
    let password = base64::encode(bytes);
    entry.set_password(&password)?;
    Ok(())
}

pub fn get_priv_key_from_os(key_name: &str) -> Result<SigningKey> {
    let entry = keyring::Entry::new("rover", key_name);
    let priv_bytes = base64::decode(entry.get_password()?)?;
    Ok(SigningKey::from_bytes(&priv_bytes).expect("error"))
}

pub fn save_key_to_memory_from_mmseed(mmseed: &str, name: &str) -> Result<()> {
    let priv_key = crate::keys::key_from_mnemonic(mmseed)?;
    save_key_to_memory(&priv_key.to_bytes(), name)?;
    Ok(())
}

pub fn save_key_to_memory(bytes: &[u8], name: &str) -> Result<()> {
    let mut keyring = OS_KEYRING.lock().expect("Error");
    keyring.insert(name.into(), bytes.to_vec());
    Ok(())
}

pub fn get_priv_key_from_memory(key_name: &str) -> Result<SigningKey> {
    let keyring = OS_KEYRING.lock().expect("Error");
    let priv_bytes = keyring
        .get(key_name)
        .ok_or_else(|| Error::Custom("key is not present in memory".into()))?;
    Ok(SigningKey::from_bytes(priv_bytes).expect("error"))
}

pub fn from_pk_to_bech32_address<K>(pub_key: ExtendedPublicKey<K>, prefix: &str) -> Result<String>
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

pub fn save_private_key_os_backend_from_mnemonic(keyname: &str, mm: &str) -> Result<()> {
    let priv_key = crate::keys::key_from_mnemonic(mm)?;

    let entry = keyring::Entry::new("rover", keyname);

    let password = base64::encode(priv_key.to_bytes());
    entry.set_password(&password)?;

    Ok(())
}
