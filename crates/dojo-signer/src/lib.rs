use std::env;

use async_trait::async_trait;
use starknet::core::crypto::Signature;
use starknet::core::types::FieldElement;
use starknet::signers::{LocalWallet, Signer, SigningKey, VerifyingKey};

#[derive(Debug, Clone)]
pub struct EnvLocalWallet {
    inner: LocalWallet,
}

impl EnvLocalWallet {
    pub fn from_env() -> anyhow::Result<Self> {
        let private_key_str = env::var("STARK_PRIVATE_KEY")?;
        let private_key = FieldElement::from_hex_be(&private_key_str)?;

        Ok(Self {
            inner: LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key)),
        })
    }
}

#[async_trait]
impl Signer for EnvLocalWallet {
    type GetPublicKeyError = <LocalWallet as Signer>::GetPublicKeyError;
    type SignError = <LocalWallet as Signer>::SignError;

    async fn get_public_key(&self) -> Result<VerifyingKey, Self::GetPublicKeyError> {
        self.inner.get_public_key().await
    }

    async fn sign_hash(&self, hash: &FieldElement) -> Result<Signature, Self::SignError> {
        self.inner.sign_hash(hash).await
    }
}
