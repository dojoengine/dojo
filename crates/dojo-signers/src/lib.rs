use std::env;

use starknet::core::types::FieldElement;
use starknet::signers::{LocalWallet, SigningKey};

pub trait FromEnv {
    fn from_env() -> anyhow::Result<Self>
    where
        Self: Sized;
}

impl FromEnv for LocalWallet {
    fn from_env() -> anyhow::Result<Self> {
        let private_key_str = env::var("STARK_PRIVATE_KEY")?;
        let private_key = FieldElement::from_hex_be(&private_key_str)?;

        Ok(LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key)))
    }
}
