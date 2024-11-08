use std::fmt::Display;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::builder::PossibleValue;
use clap::ValueEnum;
use katana_primitives::block::{BlockHash, BlockHashOrNumber, BlockNumber};
use katana_primitives::genesis::json::GenesisJson;
use katana_primitives::genesis::Genesis;

pub fn parse_seed(seed: &str) -> [u8; 32] {
    let seed = seed.as_bytes();

    if seed.len() >= 32 {
        unsafe { *(seed[..32].as_ptr() as *const [u8; 32]) }
    } else {
        let mut actual_seed = [0u8; 32];
        seed.iter().enumerate().for_each(|(i, b)| actual_seed[i] = *b);
        actual_seed
    }
}

/// Used as clap value parser for [Genesis].
pub fn parse_genesis(value: &str) -> Result<Genesis> {
    let path = PathBuf::from(shellexpand::full(value)?.into_owned());
    let genesis = Genesis::try_from(GenesisJson::load(path)?)?;
    Ok(genesis)
}

/// If the value starts with `0x`, it is parsed as a [`BlockHash`], otherwise as a [`BlockNumber`].
pub fn parse_block_hash_or_number(value: &str) -> Result<BlockHashOrNumber> {
    if value.starts_with("0x") {
        Ok(BlockHashOrNumber::Hash(BlockHash::from_hex(value)?))
    } else {
        let num = value.parse::<BlockNumber>().context("could not parse block number")?;
        Ok(BlockHashOrNumber::Num(num))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogFormat {
    Json,
    Full,
}

impl ValueEnum for LogFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Json, Self::Full]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Json => Some(PossibleValue::new("json")),
            Self::Full => Some(PossibleValue::new("full")),
        }
    }
}

impl Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Full => write!(f, "full"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_genesis_file() {
        let path = "./tests/test-data/genesis.json";
        parse_genesis(path).unwrap();
    }
}
