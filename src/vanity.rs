use crate::keys::mnemonic_to_cosmos_addr;
use crate::Result;
use anyhow::Context;
use bip32::{secp256k1::elliptic_curve::rand_core::OsRng, Language, Mnemonic};
use rayon::prelude::*;

pub fn find_parallel(vanity_prefix: &str, coin_type: &u64) -> Result<(String, Mnemonic)> {
    // https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32
    (!vanity_prefix.contains("1bio"))
        .then_some(())
        .context("contains invalid char")?;

    let hrp = "1".to_owned();

    let mm = rayon::iter::repeat(())
        .into_par_iter()
        .find_map_any(|_| {
            let mnemonic = Mnemonic::random(OsRng, Language::English);
            let address = mnemonic_to_cosmos_addr(&mnemonic, &hrp, coin_type).ok()?;
            address
                .starts_with(&(hrp.clone() + "1" + vanity_prefix))
                .then_some(mnemonic)
        })
        .context("could not find any")?;

    Ok((mnemonic_to_cosmos_addr(&mm, "cosmos", coin_type)?, mm))
}
