use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::FieldElement;
use starknet::providers::Provider;
use starknet::signers::{LocalWallet, SigningKey};
use toml::Value;

#[derive(Debug, Args)]
pub struct AccountOptions {
    #[arg(long)]
    #[arg(conflicts_with = "keystore_path")]
    pub private_key: Option<String>,

    #[arg(long)]
    pub account_address: Option<FieldElement>,

    #[arg(long)]
    pub keystore_path: Option<String>,

    #[arg(long)]
    pub keystore_password: Option<String>,
}

impl AccountOptions {
    pub async fn account<P>(
        &self,
        provider: P,
        env_metadata: Option<&Value>,
    ) -> Result<SingleOwnerAccount<P, LocalWallet>>
    where
        P: Provider + Send + Sync + 'static,
    {
        let signer = self.signer(env_metadata)?;
        let account_address = self.account_address(env_metadata)?;

        let chain_id = provider.chain_id().await?;

        Ok(SingleOwnerAccount::new(provider, signer, account_address, chain_id))
    }

    fn signer(&self, env_metadata: Option<&Value>) -> Result<LocalWallet> {
        if let Some(private_key) = self
            .private_key
            .as_deref()
            .or_else(|| {
                env_metadata.and_then(|env| env.get("private_key").and_then(|v| v.as_str()))
            })
            .or(std::env::var("DOJO_PRIVATE_KEY").ok().as_deref())
        {
            return Ok(LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
                FieldElement::from_str(private_key)?,
            )));
        }

        if let Some(path) = &self.keystore_path {
            if let Some(password) = self
                .keystore_password
                .as_deref()
                .or_else(|| {
                    env_metadata
                        .and_then(|env| env.get("keystore_password").and_then(|v| v.as_str()))
                })
                .or(std::env::var("DOJO_KEYSTORE_PASSWORD").ok().as_deref())
            {
                return Ok(LocalWallet::from_signing_key(SigningKey::from_keystore(
                    path, password,
                )?));
            } else {
                return Err(anyhow!("Keystore path is specified but password is not."));
            }
        }

        Err(anyhow!(
            "Could not find private key. Please specify the private key or path to the keystore \
             file."
        ))
    }

    fn account_address(&self, env_metadata: Option<&Value>) -> Result<FieldElement> {
        if let Some(address) = self.account_address {
            Ok(address)
        } else if let Some(address) = env_metadata.and_then(|env| {
            env.get("account_address")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or(std::env::var("DOJO_ACCOUNT_ADDRESS").ok())
        }) {
            Ok(FieldElement::from_str(&address)?)
        } else {
            Err(anyhow!(
                "Could not find account address. Please specify it with --account-address or in \
                 the environment config."
            ))
        }
    }
}
