use std::path::PathBuf;

use katana_primitives::genesis::json::GenesisJsonWithBasePath;
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

pub fn parse_genesis(path: &str) -> Result<Genesis, anyhow::Error> {
    let path = PathBuf::from(shellexpand::full(path)?.into_owned());
    let genesis = Genesis::try_from(GenesisJsonWithBasePath::new(path)?)?;
    Ok(genesis)
}
