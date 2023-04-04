use starknet::signers::{LocalWallet, VerifyingKey, SigningKey, Signer};
use starknet::core::{crypto::{Signature}, types::FieldElement};
use dotenv::dotenv;
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::env;
use async_trait::async_trait;

lazy_static! {
    static ref STARK_PRIVATE_KEY: Mutex<Option<SigningKey>> = Mutex::new(None);
}

#[derive(Debug, Clone)]
pub struct EnvLocalWallet {
    inner_wallet: LocalWallet,
}

impl EnvLocalWallet {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv().ok();

        let private_key = {
            let mut private_key_guard = STARK_PRIVATE_KEY.lock().unwrap();
            match private_key_guard.clone() {
                Some(key) => key,
                None => {
                    let private_key_str = env::var("STARK_PRIVATE_KEY")?;
                    let private_key = FieldElement::from_hex_be(
                        &private_key_str,
                    )
                    .unwrap();
            
                    let key = SigningKey::from_secret_scalar(private_key);
                    *private_key_guard = Some(key.clone());
                    key
                }
            }
        };

        let local_wallet = LocalWallet::from_signing_key(private_key);
        Ok(Self { inner_wallet: local_wallet })
    }
}

#[async_trait]
impl Signer for EnvLocalWallet {
    type GetPublicKeyError = <LocalWallet as Signer>::GetPublicKeyError;
    type SignError = <LocalWallet as Signer>::SignError;

    async fn get_public_key(&self) -> Result<VerifyingKey, Self::GetPublicKeyError> {
        self.inner_wallet.get_public_key().await
    }

    async fn sign_hash(&self, hash: &FieldElement) -> Result<Signature, Self::SignError> {
        self.inner_wallet.sign_hash(hash).await
    }
}