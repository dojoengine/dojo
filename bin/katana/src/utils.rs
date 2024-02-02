use std::path::PathBuf;

use anyhow::Context;
use katana_primitives::genesis::json::{from_base64, GenesisJson};
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
pub fn parse_genesis(value: &str) -> Result<Genesis, anyhow::Error> {
    // if the value is a base64 string, we assume it already resolves and includes
    // the classes artifacts and we can just deserialize into the main genesis type directly
    match value.strip_prefix("base64:") {
        Some(data) => {
            let json = from_base64(data.as_bytes())?;
            let genesis = Genesis::try_from(json).context("Parsing genesis file")?;
            Ok(genesis)
        }

        None => {
            let path = PathBuf::from(shellexpand::full(value)?.into_owned());
            let genesis = Genesis::try_from(GenesisJson::load(path)?)?;
            Ok(genesis)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_base64_genesis() {
        let base64 = std::fs::read_to_string("./tests/test-data/base64-genesis.txt").unwrap();
        assert!(parse_genesis(&base64).is_ok())
    }

    #[test]
    fn parse_genesis_file() {
        let path = "./tests/test-data/genesis.json";
        parse_genesis(path).unwrap();
    }
}
