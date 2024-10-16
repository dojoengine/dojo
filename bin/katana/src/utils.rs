use std::path::PathBuf;

use anyhow::{Context, Result};
use katana_primitives::block::{BlockHash, BlockHashOrNumber};
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

pub fn parse_block_hash_or_number(value: &str) -> Result<BlockHashOrNumber> {
    if value.starts_with("0x") {
        let hash = BlockHash::from_hex(value)?;
        Ok(BlockHashOrNumber::Hash(hash))
    } else {
        let num = value.parse::<u64>().context("could not parse block number")?;
        Ok(BlockHashOrNumber::Num(num))
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
