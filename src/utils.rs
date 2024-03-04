use base64::prelude::{Engine as _, BASE64_STANDARD};
use bech32::{Bech32, Hrp};
use cosmos_sdk_proto::prost_wkt_types::MessageSerde;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::Result;

pub fn write_base64_to_file<T>(t: &T, file_name: &str) -> Result<()>
where
    T: MessageSerde,
{
    let base64_str = BASE64_STANDARD.encode(t.try_encoded()?);
    std::fs::write(file_name, base64_str)?;
    Ok(())
}

pub fn write_binary_to_file<T>(t: &T, file_name: &str) -> Result<()>
where
    T: MessageSerde,
{
    std::fs::write(file_name, t.try_encoded()?)?;
    Ok(())
}

pub fn read_bytes_from_file(file_name: &str) -> Result<Vec<u8>> {
    Ok(std::fs::read(file_name)?)
}

pub fn read_base64_from_file(file_name: &str) -> Result<Vec<u8>> {
    let s = std::fs::read_to_string(file_name)?;
    Ok(BASE64_STANDARD.decode(s.trim())?)
}

pub fn read_from_base64<T>(base64: &str) -> Result<T>
where
    T: MessageSerde + Default + Clone,
{
    read_from_bytes(&BASE64_STANDARD.decode(base64.trim())?)
}

pub fn read_from_bytes<T>(bytes: &[u8]) -> Result<T>
where
    T: MessageSerde + Default + Clone,
{
    let msg_dyn = T::default().new_instance(bytes.to_vec())?;

    Ok(msg_dyn.downcast_ref::<T>().unwrap().clone())
}

pub fn bech32(address: &str, prefix: &str) -> Result<String> {
    let (_, bytes) = bech32::decode(address)?;
    Ok(bech32::encode::<Bech32>(Hrp::parse(prefix)?, &bytes)?)
}

pub fn parse_dec_amount(st: &str, precision: usize) -> Result<u128> {
    Ok(st
        .chars()
        .rev()
        .skip(precision)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .unwrap_or_default())
}

pub fn read_data_from_yaml<T>(path: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    Ok(serde_yaml::from_reader(reader)?)
}

pub fn write_data_as_yaml<T>(path: &str, value: T) -> Result<()>
where
    T: Serialize,
{
    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    Ok(serde_yaml::to_writer(writer, &value)?)
}

pub fn update_chain(chain_name: &str, path: &str, value: Value, file_path: &str) -> Result<()> {
    let mut chains: Value = crate::utils::read_data_from_yaml(file_path)?;

    chains
        .pointer_mut(&format!("/{chain_name}{path}"))
        .map(|v| *v = value)
        .expect("error");

    write_data_as_yaml(file_path, chains)?;

    Ok(())
}
