use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::FieldElement;
use starknet::providers::Provider;
use starknet::signers::{LocalWallet, SigningKey};
use toml::Value;

#[derive(Debug, Args)]
#[command(next_help_heading = "Account options")]
pub struct AccountOptions {
    #[arg(long)]
    pub account_address: Option<FieldElement>,

    #[arg(long)]
    #[arg(requires = "account_address")]
    #[arg(conflicts_with = "keystore_path")]
    #[arg(help_heading = "Signer options - RAW")]
    #[arg(help = "The raw private key associated with the account contract.")]
    pub private_key: Option<String>,

    #[arg(long = "keystore")]
    #[arg(value_name = "PATH")]
    #[arg(help_heading = "Signer options - KEYSTORE")]
    #[arg(help = "Use the keystore in the given folder or file.")]
    pub keystore_path: Option<String>,

    #[arg(long = "password")]
    #[arg(value_name = "PASSWORD")]
    #[arg(requires = "keystore_path")]
    #[arg(help_heading = "Signer options - KEYSTORE")]
    #[arg(help = "The keystore password. Used with --keystore.")]
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
        let account_address = self.account_address(env_metadata)?;
        let signer = self.signer(env_metadata)?;

        let chain_id =
            provider.chain_id().await.with_context(|| "Failed to retrieve network chain id.")?;

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
