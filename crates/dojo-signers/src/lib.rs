use std::env;

use starknet::core::types::FieldElement;
use starknet::signers::{LocalWallet, SigningKey};

pub trait FromEnv {
    fn from_env() -> anyhow::Result<Self>
    where
        Self: Sized;
}

pub trait FromKeystore {
    fn from_keystore() -> anyhow::Result<Self>
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

impl FromKeystore for LocalWallet {
    fn from_keystore<P>(keystore_path: P, password: &str) -> anyhow::Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        let private_key_str = SigningKey::from_keystore(keystore_path, password)?;
        let private_key = FieldElement::from_hex_be(&private_key_str)?;

        Ok(LocalWalletKeystore::from_signing_key(SigningKey::from_keystore(
            &keystore_path,
            &password,
        )?))
    }
}
