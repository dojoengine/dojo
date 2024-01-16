use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::FieldElement;
use starknet::providers::Provider;
use starknet::signers::{LocalWallet, SigningKey};

use super::{
    DOJO_ACCOUNT_ADDRESS_ENV_VAR, DOJO_KEYSTORE_PASSWORD_ENV_VAR, DOJO_KEYSTORE_PATH_ENV_VAR,
    DOJO_PRIVATE_KEY_ENV_VAR,
};

#[derive(Debug, Args)]
#[command(next_help_heading = "Account options")]
// INVARIANT:
// - For commandline: we can either specify `private_key` or `keystore_path` along with
//   `keystore_password`. This is enforced by Clap.
// - For `Scarb.toml`: if both private_key and keystore are specified in `Scarb.toml` private_key
//   will take priority
pub struct AccountOptions {
    #[arg(long, env = DOJO_ACCOUNT_ADDRESS_ENV_VAR)]
    pub account_address: Option<FieldElement>,

    #[arg(long, env = DOJO_PRIVATE_KEY_ENV_VAR)]
    #[arg(conflicts_with = "keystore_path")]
    #[arg(help_heading = "Signer options - RAW")]
    #[arg(help = "The raw private key associated with the account contract.")]
    pub private_key: Option<String>,

    #[arg(long = "keystore", env = DOJO_KEYSTORE_PATH_ENV_VAR)]
    #[arg(value_name = "PATH")]
    #[arg(help_heading = "Signer options - KEYSTORE")]
    #[arg(help = "Use the keystore in the given folder or file.")]
    pub keystore_path: Option<String>,

    #[arg(long = "password", env = DOJO_KEYSTORE_PASSWORD_ENV_VAR)]
    #[arg(value_name = "PASSWORD")]
    #[arg(help_heading = "Signer options - KEYSTORE")]
    #[arg(help = "The keystore password. Used with --keystore.")]
    pub keystore_password: Option<String>,

    #[arg(long)]
    #[arg(help = "Use legacy account (cairo0 account)")]
    pub legacy: bool,
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

        let encoding = if self.legacy { ExecutionEncoding::Legacy } else { ExecutionEncoding::New };

        Ok(SingleOwnerAccount::new(provider, signer, account_address, chain_id, encoding))
    }

    fn signer(&self, env_metadata: Option<&Environment>) -> Result<LocalWallet> {
        if let Some(private_key) =
            self.private_key.as_deref().or_else(|| env_metadata.and_then(|env| env.private_key()))
        {
            return Ok(LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
                FieldElement::from_str(private_key)?,
            )));
        }

        if let Some(path) = &self
            .keystore_path
            .as_deref()
            .or_else(|| env_metadata.and_then(|env| env.keystore_path()))
        {
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
    use std::str::FromStr;

    use clap::Parser;
    use starknet::accounts::{Call, ExecutionEncoder};
    use starknet::signers::{LocalWallet, Signer, SigningKey};
    use starknet_crypto::FieldElement;

    use super::{
        AccountOptions, DOJO_ACCOUNT_ADDRESS_ENV_VAR, DOJO_KEYSTORE_PASSWORD_ENV_VAR,
        DOJO_PRIVATE_KEY_ENV_VAR,
    };

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
    fn account_address_from_args() {
        let cmd = Command::parse_from(["sozo", "--account-address", "0x0"]);
        assert_eq!(
            cmd.account.account_address(None).unwrap(),
            FieldElement::from_hex_be("0x0").unwrap()
        );
    }

    #[test]
    fn account_address_from_env_metadata() {
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
    fn account_address_from_both() {
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
    fn account_address_from_neither() {
        let cmd = Command::parse_from([""]);
        assert!(cmd.account.account_address(None).is_err());
    }

    #[tokio::test]
    async fn private_key_from_args() {
        let private_key = "0x1";

        let cmd =
            Command::parse_from(["sozo", "--account-address", "0x0", "--private-key", private_key]);
        let result_wallet = cmd.account.signer(None).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            FieldElement::from_str(private_key).unwrap(),
        ));

        let result_public_key = result_wallet.get_public_key().await.unwrap();
        let expected_public_key = expected_wallet.get_public_key().await.unwrap();
        assert!(result_public_key.scalar() == expected_public_key.scalar());
    }

    #[tokio::test]
    async fn private_key_from_env_metadata() {
        let private_key = "0x1";
        let env_metadata = dojo_world::metadata::Environment {
            private_key: Some(private_key.to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--account-address", "0x0"]);
        let result_wallet = cmd.account.signer(Some(&env_metadata)).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            FieldElement::from_str(private_key).unwrap(),
        ));

        let result_public_key = result_wallet.get_public_key().await.unwrap();
        let expected_public_key = expected_wallet.get_public_key().await.unwrap();
        assert!(result_public_key.scalar() == expected_public_key.scalar());
    }

    #[tokio::test]
    async fn keystore_path_and_keystore_password_from_args() {
        let keystore_path = "./tests/test_data/keystore/test.json";
        let keystore_password = "dojoftw";
        let private_key = "0x1";

        let cmd = Command::parse_from([
            "sozo",
            "--keystore",
            keystore_path,
            "--password",
            keystore_password,
        ]);
        let result_wallet = cmd.account.signer(None).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            FieldElement::from_str(private_key).unwrap(),
        ));

        let result_public_key = result_wallet.get_public_key().await.unwrap();
        let expected_public_key = expected_wallet.get_public_key().await.unwrap();
        assert!(result_public_key.scalar() == expected_public_key.scalar());
    }

    #[tokio::test]
    async fn keystore_path_from_env_metadata() {
        let keystore_path = "./tests/test_data/keystore/test.json";
        let keystore_password = "dojoftw";

        let private_key = "0x1";
        let env_metadata = dojo_world::metadata::Environment {
            keystore_path: Some(keystore_path.to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--password", keystore_password]);
        let result_wallet = cmd.account.signer(Some(&env_metadata)).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            FieldElement::from_str(private_key).unwrap(),
        ));

        let result_public_key = result_wallet.get_public_key().await.unwrap();
        let expected_public_key = expected_wallet.get_public_key().await.unwrap();
        assert!(result_public_key.scalar() == expected_public_key.scalar());
    }

    #[tokio::test]
    async fn keystore_password_from_env_metadata() {
        let keystore_path = "./tests/test_data/keystore/test.json";
        let keystore_password = "dojoftw";
        let private_key = "0x1";

        let env_metadata = dojo_world::metadata::Environment {
            keystore_password: Some(keystore_password.to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--keystore", keystore_path]);
        let result_wallet = cmd.account.signer(Some(&env_metadata)).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            FieldElement::from_str(private_key).unwrap(),
        ));

        let result_public_key = result_wallet.get_public_key().await.unwrap();
        let expected_public_key = expected_wallet.get_public_key().await.unwrap();
        assert!(result_public_key.scalar() == expected_public_key.scalar());
    }

    #[test]
    fn dont_allow_both_private_key_and_keystore() {
        let keystore_path = "./tests/test_data/keystore/test.json";
        let private_key = "0x1";
        assert!(Command::try_parse_from([
            "sozo",
            "--keystore",
            keystore_path,
            "--private_key",
            private_key,
        ])
        .is_err());
    }

    #[katana_runner::katana_test]
    async fn legacy_flag_works_as_expected() {
        let cmd = Command::parse_from([
            "sozo",
            "--legacy",
            "--account-address",
            "0x0",
            "--private-key",
            "0x1",
        ]);
        let dummy_call = vec![Call {
            to: FieldElement::from_hex_be("0x0").unwrap(),
            selector: FieldElement::from_hex_be("0x1").unwrap(),
            calldata: vec![
                FieldElement::from_hex_be("0x2").unwrap(),
                FieldElement::from_hex_be("0x3").unwrap(),
            ],
        }];

        // HACK: SingleOwnerAccount doesn't expose a way to check `encoding` type used in struct, so
        // checking it by encoding a dummy call and checking which method it used to encode the call
        let account = cmd.account.account(runner.provider(), None).await.unwrap();
        let result = account.encode_calls(&dummy_call);
        // 0x0 is the data offset.
        assert!(*result.get(3).unwrap() == FieldElement::from_hex_be("0x0").unwrap());
    }

    #[katana_runner::katana_test]
    async fn without_legacy_flag_works_as_expected() {
        let cmd = Command::parse_from(["sozo", "--account-address", "0x0", "--private-key", "0x1"]);
        let dummy_call = vec![Call {
            to: FieldElement::from_hex_be("0x0").unwrap(),
            selector: FieldElement::from_hex_be("0x1").unwrap(),
            calldata: vec![
                FieldElement::from_hex_be("0xf2").unwrap(),
                FieldElement::from_hex_be("0xf3").unwrap(),
            ],
        }];

        // HACK: SingleOwnerAccount doesn't expose a way to check `encoding` type used in struct, so
        // checking it by encoding a dummy call and checking which method it used to encode the call
        let account = cmd.account.account(runner.provider(), None).await.unwrap();
        let result = account.encode_calls(&dummy_call);
        // 0x2 is the Calldata len.
        assert!(*result.get(3).unwrap() == FieldElement::from_hex_be("0x2").unwrap());
    }

    #[test]
    fn keystore_path_without_keystore_password() {
        let keystore_path = "./tests/test_data/keystore/test.json";

        let cmd = Command::parse_from(["sozo", "--keystore", keystore_path]);
        let result = cmd.account.signer(None);

        assert!(result.is_err());
    }

    #[test]
    fn signer_without_pk_or_keystore() {
        let cmd = Command::parse_from(["sozo"]);
        let result = cmd.account.signer(None);

        assert!(result.is_err());
    }
}
