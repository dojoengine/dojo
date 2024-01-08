use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::FieldElement;
use starknet::providers::Provider;
use starknet::signers::{LocalWallet, SigningKey};

use super::{
    DOJO_ACCOUNT_ADDRESS_ENV_VAR, DOJO_KEYSTORE_PASSWORD_ENV_VAR, DOJO_PRIVATE_KEY_ENV_VAR,
};

#[derive(Debug, Args)]
#[command(next_help_heading = "Account options")]
pub struct AccountOptions {
    #[arg(long, env = DOJO_ACCOUNT_ADDRESS_ENV_VAR)]
    pub account_address: Option<FieldElement>,

    #[arg(long, env = DOJO_PRIVATE_KEY_ENV_VAR)]
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

    #[arg(long = "password", env = DOJO_KEYSTORE_PASSWORD_ENV_VAR)]
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
        env_metadata: Option<&Environment>,
    ) -> Result<SingleOwnerAccount<P, LocalWallet>>
    where
        P: Provider + Send + Sync,
    {
        let account_address = self.account_address(env_metadata)?;
        let signer = self.signer(env_metadata)?;

        let chain_id =
            provider.chain_id().await.with_context(|| "Failed to retrieve network chain id.")?;

        Ok(SingleOwnerAccount::new(
            provider,
            signer,
            account_address,
            chain_id,
            ExecutionEncoding::New,
        ))
    }

    fn signer(&self, env_metadata: Option<&Environment>) -> Result<LocalWallet> {
        if let Some(private_key) =
            self.private_key.as_deref().or_else(|| env_metadata.and_then(|env| env.private_key()))
        {
            return Ok(LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
                FieldElement::from_str(private_key)?,
            )));
        }

        if let Some(path) = &self.keystore_path {
            if let Some(password) = self
                .keystore_password
                .as_deref()
                .or_else(|| env_metadata.and_then(|env| env.keystore_password()))
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

    fn account_address(&self, env_metadata: Option<&Environment>) -> Result<FieldElement> {
        if let Some(address) = self.account_address {
            Ok(address)
        } else if let Some(address) = env_metadata.and_then(|env| env.account_address()) {
            Ok(FieldElement::from_str(address)?)
        } else {
            Err(anyhow!(
                "Could not find account address. Please specify it with --account-address or in \
                 the environment config."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AccountOptions;
    use super::{
        DOJO_ACCOUNT_ADDRESS_ENV_VAR, DOJO_KEYSTORE_PASSWORD_ENV_VAR, DOJO_PRIVATE_KEY_ENV_VAR,
    };
    use clap::Parser;
    use starknet_crypto::FieldElement;

    #[derive(clap::Parser, Debug)]
    struct Command {
        #[clap(flatten)]
        pub account: AccountOptions,
    }

    #[test]
    fn account_address_read_from_env_variable() {
        std::env::set_var(DOJO_ACCOUNT_ADDRESS_ENV_VAR, "0x0");

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.account.account_address, Some(FieldElement::from_hex_be("0x0").unwrap()));
    }

    #[test]
    fn private_key_read_from_env_variable() {
        std::env::set_var(DOJO_PRIVATE_KEY_ENV_VAR, "private_key");

        let cmd = Command::parse_from(["sozo", "--account-address", "0x0"]);
        assert_eq!(cmd.account.private_key, Some("private_key".to_owned()));
    }

    #[test]
    fn keystore_path_read_from_env_variable() {
        std::env::set_var(DOJO_KEYSTORE_PASSWORD_ENV_VAR, "keystore_password");

        let cmd = Command::parse_from(["sozo", "--keystore", "./some/path"]);
        assert_eq!(cmd.account.keystore_password, Some("keystore_password".to_owned()));
    }

    #[test]
    fn address_from_args() {
        let env_metadata = dojo_world::metadata::Environment::default();

        let cmd = Command::parse_from(["sozo", "--address", "0x0"]);
        assert_eq!(
            cmd.account.account_address(Some(&env_metadata)).unwrap(),
            FieldElement::from_hex_be("0x0").unwrap()
        );
    }

    #[test]
    fn address_from_env_metadata() {
        let env_metadata = dojo_world::metadata::Environment {
            account_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);

        assert_eq!(
            cmd.account.account_address(Some(&env_metadata)).unwrap(),
            FieldElement::from_hex_be("0x0").unwrap()
        );
    }

    #[test]
    fn address_from_both() {
        let env_metadata = dojo_world::metadata::Environment {
            account_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--account-address", "0x1"]);

        assert_eq!(
            cmd.account.account_address(Some(&env_metadata)).unwrap(),
            FieldElement::from_hex_be("0x1").unwrap()
        );
    }

    #[test]
    fn address_from_neither() {
        let env_metadata = dojo_world::metadata::Environment::default();

        let cmd = Command::parse_from([""]);

        assert!(cmd.account.account_address(Some(&env_metadata)).is_err(),);
    }
}
