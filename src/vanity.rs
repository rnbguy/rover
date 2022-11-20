use std::{
    sync::{atomic::Ordering, Arc, RwLock},
    time::SystemTime,
};

use crate::keys::mnemonic_to_cosmos_addr;
use crate::Result;
use anyhow::Context;
use bip32::{secp256k1::elliptic_curve::rand_core::OsRng, Language, Mnemonic};
use rayon::prelude::*;
use std::sync::atomic::AtomicU64;

pub fn find_parallel(vanity_prefix: &str, coin_type: &u64) -> Result<(String, Mnemonic)> {
    // https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki#bech32
    (!vanity_prefix.contains("1bio"))
        .then_some(())
        .context("contains invalid char")?;

    let hrp = "1".to_owned();

    let ts = Arc::new(RwLock::new(SystemTime::now()));

    let ix = AtomicU64::new(0);
    let periods = AtomicU64::new(0);

    let period_secs = 60;

    let probability = 1. - (1. / 32_f64.powi(vanity_prefix.len() as i32));

    let mm = rayon::iter::repeat(())
        .into_par_iter()
        .find_map_any(|_| {
            let mnemonic = Mnemonic::random(OsRng, Language::English);
            let address = mnemonic_to_cosmos_addr(&mnemonic, &hrp, coin_type).ok()?;

            ix.fetch_add(1, Ordering::Relaxed);

            let checkpoint = ts
                .read()
                .unwrap()
                .elapsed()
                .expect("elapsed error")
                .as_secs()
                > period_secs;
            if checkpoint {
                periods.fetch_add(1, Ordering::Relaxed);
                let mut ts_ = ts.write().unwrap();
                *ts_ = SystemTime::now();
                println!(
                    "processing {:?} | current speed {:.0?}/s | next probablity {:.3?}",
                    ix,
                    ix.load(Ordering::Relaxed) as f64
                        / (period_secs * periods.load(Ordering::Relaxed)) as f64,
                    1. - (probability).powi(ix.load(Ordering::Relaxed) as i32)
                );
            }

            address
                .starts_with(&(hrp.clone() + "1" + vanity_prefix))
                .then_some(mnemonic)
        })
        .context("could not find any")?;

    Ok((mnemonic_to_cosmos_addr(&mm, "cosmos", coin_type)?, mm))
}
