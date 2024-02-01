use std::fs::File;
use std::io::BufReader;
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

pub fn parse_genesis(value: &str) -> Result<Genesis, anyhow::Error> {
    // if the value is a base64 string, we assume it already resolves and includes
    // the classes artifacts and we can just deserialize into the main genesis type directly
    match value.strip_prefix("base64:") {
        Some(data) => {
            let json = from_base64(data.as_bytes())?;
            let genesis = json.into_genesis_unchecked().context("Parsing genesis file")?;
            Ok(genesis)
        }

        None => {
            let path = PathBuf::from(shellexpand::full(value)?.into_owned());
            let file = BufReader::new(File::open(&path)?);

            let json: GenesisJson = serde_json::from_reader(file)?;
            let genesis: Genesis = json.into_genesis(path)?;
            Ok(genesis)
        }
    }
}
