use anyhow::Context;
use bip32::DerivationPath;
use ledger_transport::{APDUCommand, APDUErrorCode};
use ledger_transport_hid::{hidapi, TransportNativeHID};
use serde_json::Value;

use crate::Result;

// https://github.com/cosmos/ledger-cosmos#apdu-specifications
// https://github.com/cosmos/ledger-cosmos/blob/main/docs/APDUSPEC.md
// https://github.com/cosmos/ledger-cosmos/blob/main/docs/TXSPEC.md

pub fn apdu_get_version() -> APDUCommand<Vec<u8>> {
    APDUCommand {
        cla: 0x55,
        ins: 0x00,
        p1: 0x00,
        p2: 0x00,
        data: vec![],
    }
}

pub fn process_version(bytes: &[u8]) -> Result<(u8, u16, u16, u16, u16)> {
    Ok((
        bytes[0],
        u16::from_le_bytes(bytes[1..3].try_into().context("no error")?),
        u16::from_le_bytes(bytes[3..5].try_into().context("no error")?),
        u16::from_le_bytes(bytes[5..7].try_into().context("no error")?),
        u16::from_le_bytes(bytes[7..9].try_into().context("no error")?),
    ))
}

pub fn apdu_ins_get_addr_secp256k1(
    hrp: &str,
    derivation_path: &DerivationPath,
    show_address: bool,
) -> APDUCommand<Vec<u8>> {
    let mut bytes = vec![];

    let hrp_bytes = hrp.as_bytes().to_vec();

    bytes.push(hrp_bytes.len() as u8);
    bytes.extend(hrp_bytes);

    for child in derivation_path.as_ref() {
        bytes.extend(child.0.to_le_bytes());
    }

    APDUCommand {
        cla: 0x55,
        ins: 0x04,
        p1: u8::from(show_address),
        p2: 0x00,
        data: bytes,
    }
}

pub fn process_pub_key(bytes: &[u8]) -> Result<(Vec<u8>, String)> {
    Ok((
        bytes[..33].to_vec(),
        String::from_utf8(bytes[33..].to_vec()).context("couldn't convert to string")?,
    ))
}

pub fn apdu_sign_secp256k1(
    payload: Value,
    derivation_path: &DerivationPath,
) -> Result<Vec<APDUCommand<Vec<u8>>>> {
    let mut commands = vec![];

    let mut bytes = vec![];

    for child in derivation_path.as_ref() {
        bytes.extend(child.0.to_le_bytes());
    }

    commands.push(APDUCommand {
        cla: 0x55,
        ins: 0x02,
        p1: 0x00,
        p2: 0x00,
        data: bytes,
    });

    let payload_str = serde_json::to_string(&payload)?;

    let payload = payload_str.into_bytes();
    let chunks = payload.as_slice().chunks(64);

    let n = chunks.len();

    for (i, chunks) in chunks.into_iter().enumerate() {
        let desc = if i + 1 == n { 0x02 } else { 0x01 };

        commands.push(APDUCommand {
            cla: 0x55,
            ins: 0x02,
            p1: desc,
            p2: 0x00,
            data: chunks.to_vec(),
        });
    }
    Ok(commands)
}

pub async fn get_version() -> Result<(u8, u16, u16, u16, u16)> {
    let transport = TransportNativeHID::new(&hidapi::HidApi::new()?)?;

    let command = apdu_get_version();

    let resp = transport.exchange(&command)?;

    match resp.error_code() {
        Ok(APDUErrorCode::NoError) => process_version(resp.data()),
        _ => Err(anyhow::anyhow!("{:?}", resp.error_code())),
    }
}

pub async fn get_pub_key(
    hrp: &str,
    derivation_path: &DerivationPath,
    show_address: bool,
) -> Result<Vec<u8>> {
    let transport = TransportNativeHID::new(&hidapi::HidApi::new()?)?;
    let command = apdu_ins_get_addr_secp256k1(hrp, derivation_path, show_address);

    let resp = transport.exchange(&command)?;

    match resp.error_code() {
        Ok(APDUErrorCode::NoError) => Ok(resp.data().to_vec()),
        _ => Err(anyhow::anyhow!("{:?}", resp.error_code())),
    }
}

pub fn transform_der_to_ber(bytes: &[u8]) -> Result<[u8; 64]> {
    // https://github.com/btcsuite/btcd/blob/4dc4ff7963b4fb101eaf1d201e52fdbc034389be/btcec/ecdsa/signature.go#L68
    // 0x30 <length of whole message> <0x02> <length of R> <R> 0x2 <length of S> <S>

    let (rem, val) = der_parser::der::parse_der_sequence(bytes)?;

    assert!(rem.is_empty());

    let bers = val.as_sequence()?;

    assert_eq!(bers.len(), 2);

    let r = bers[0].as_slice()?;
    let s = bers[1].as_slice()?;

    // https://github.com/tendermint/btcd/blob/80daadac05d1cd29571fccf27002d79667a88b58/btcec/signature.go#L36
    // we set s = curve_order - s, if s is greater than curve.Order() / 2.

    // let curve_order = bip32::secp256k1::Secp256k1::ORDER;

    // let mut big_s = UInt::from_be_slice(s);
    // let big_s_p = curve_order.checked_sub(&big_s).context("no checked_sub")?;

    // if big_s.ge(&big_s_p) {
    //     big_s = big_s_p;
    // }

    // let s: Vec<u8> = big_s.to_uint_array().into_iter().flat_map(|x| x.to_be_bytes()).collect();

    let r = r
        .iter()
        .skip_while(|&x| x == &0)
        .cloned()
        .collect::<Vec<_>>();
    let s = s
        .iter()
        .skip_while(|&x| x == &0)
        .cloned()
        .collect::<Vec<_>>();

    let mut fin_value = [0; 64];

    fin_value[32 - r.len()..32].copy_from_slice(&r);
    fin_value[64 - s.len()..64].copy_from_slice(&s);

    Ok(fin_value)

    // Ok(r.chain(s)
    //     .cloned()
    //     .collect::<Vec<_>>()
    //     .try_into()
    //     .expect("error"))
}

pub async fn get_signature(payload: Value, derivation_path: &DerivationPath) -> Result<[u8; 64]> {
    let commands = apdu_sign_secp256k1(payload, derivation_path)?;
    let transport = TransportNativeHID::new(&hidapi::HidApi::new()?)?;

    let resps: Vec<_> = commands
        .into_iter()
        .map(|command| transport.exchange(&command))
        .collect::<std::result::Result<_, _>>()?;

    let resp = resps.last().context("no last")?;

    match resp.error_code() {
        Ok(APDUErrorCode::NoError) => Ok(transform_der_to_ber(resp.data())?),
        _ => Err(anyhow::anyhow!("{:?}", resp.error_code())),
    }
}
