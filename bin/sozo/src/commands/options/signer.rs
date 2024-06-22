use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::core::types::FieldElement;
use starknet::signers::{LocalWallet, SigningKey};
use tracing::trace;

use super::{DOJO_KEYSTORE_PASSWORD_ENV_VAR, DOJO_KEYSTORE_PATH_ENV_VAR, DOJO_PRIVATE_KEY_ENV_VAR};

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Signer options")]
// INVARIANT:
// - For commandline: we can either specify `private_key` or `keystore_path` along with
//   `keystore_password`. This is enforced by Clap.
// - For `Scarb.toml`: if both private_key and keystore are specified in `Scarb.toml` private_key
//   will take priority
pub struct SignerOptions {
    #[arg(long, env = DOJO_PRIVATE_KEY_ENV_VAR)]
    #[arg(conflicts_with = "keystore_path")]
    #[arg(help_heading = "Signer options - RAW")]
    #[arg(help = "The raw private key associated with the account contract.")]
    #[arg(global = true)]
    #[arg(group = "signer")]
    pub private_key: Option<String>,

    #[arg(long = "keystore", env = DOJO_KEYSTORE_PATH_ENV_VAR)]
    #[arg(value_name = "PATH")]
    #[arg(help_heading = "Signer options - KEYSTORE")]
    #[arg(help = "Use the keystore in the given folder or file.")]
    #[arg(global = true)]
    #[arg(group = "signer")]
    pub keystore_path: Option<String>,

    #[arg(long = "password", env = DOJO_KEYSTORE_PASSWORD_ENV_VAR)]
    #[arg(value_name = "PASSWORD")]
    #[arg(help_heading = "Signer options - KEYSTORE")]
    #[arg(help = "The keystore password. Used with --keystore.")]
    #[arg(global = true)]
    pub keystore_password: Option<String>,
}

impl SignerOptions {
    pub fn signer(&self, env_metadata: Option<&Environment>, no_wait: bool) -> Result<LocalWallet> {
        if let Some(private_key) = self.private_key(env_metadata) {
            trace!(private_key, "Signing using private key.");
            return Ok(LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
                FieldElement::from_str(&private_key)?,
            )));
        }

        if let Some(path) = self.keystore_path(env_metadata) {
            let password = {
                if let Some(password) = self.keystore_password(env_metadata) {
                    password.to_owned()
                } else if no_wait {
                    return Err(anyhow!("Could not find password. Please specify the password."));
                } else {
                    trace!("Prompting user for keystore password.");
                    rpassword::prompt_password("Enter password: ")?
                }
            };
            let private_key = SigningKey::from_keystore(path, &password)?;
            return Ok(LocalWallet::from_signing_key(private_key));
        }

        Err(anyhow!(
            "Could not find private key. Please specify the private key or path to the keystore \
             file."
        ))
    }

    pub fn private_key(&self, env_metadata: Option<&Environment>) -> Option<String> {
        if let Some(s) = &self.private_key {
            Some(s.to_owned())
        } else {
            env_metadata.and_then(|env| env.private_key().map(|s| s.to_string()))
        }
    }

    pub fn keystore_path(&self, env_metadata: Option<&Environment>) -> Option<String> {
        if let Some(s) = &self.keystore_path {
            Some(s.to_owned())
        } else {
            env_metadata.and_then(|env| env.keystore_path().map(|s| s.to_string()))
        }
    }

    pub fn keystore_password(&self, env_metadata: Option<&Environment>) -> Option<String> {
        if let Some(s) = &self.keystore_password {
            Some(s.to_owned())
        } else {
            env_metadata.and_then(|env| env.keystore_password().map(|s| s.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use clap::Parser;
    use starknet::signers::{LocalWallet, Signer, SigningKey};
    use starknet_crypto::FieldElement;

    use super::{SignerOptions, DOJO_KEYSTORE_PASSWORD_ENV_VAR, DOJO_PRIVATE_KEY_ENV_VAR};

    #[derive(clap::Parser, Debug)]
    struct Command {
        #[clap(flatten)]
        pub signer: SignerOptions,
    }

    #[test]
    fn private_key_read_from_env_variable() {
        std::env::set_var(DOJO_PRIVATE_KEY_ENV_VAR, "private_key");

        let cmd = Command::parse_from(["sozo"]);
        assert_eq!(cmd.signer.private_key, Some("private_key".to_owned()));
    }

    #[test]
    fn keystore_path_read_from_env_variable() {
        std::env::set_var(DOJO_KEYSTORE_PASSWORD_ENV_VAR, "keystore_password");

        let cmd = Command::parse_from(["sozo", "--keystore", "./some/path"]);
        assert_eq!(cmd.signer.keystore_password, Some("keystore_password".to_owned()));
    }

    #[tokio::test]
    async fn private_key_from_args() {
        let private_key = "0x1";

        let cmd = Command::parse_from(["sozo", "--private-key", private_key]);
        let result_wallet = cmd.signer.signer(None, true).unwrap();
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

        let cmd = Command::parse_from(["sozo"]);
        let result_wallet = cmd.signer.signer(Some(&env_metadata), true).unwrap();
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
        let result_wallet = cmd.signer.signer(None, true).unwrap();
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
        let result_wallet = cmd.signer.signer(Some(&env_metadata), true).unwrap();
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
        let result_wallet = cmd.signer.signer(Some(&env_metadata), true).unwrap();
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
        let parse_result = Command::try_parse_from([
            "sozo",
            "--keystore",
            keystore_path,
            "--private_key",
            private_key,
        ]);
        assert!(parse_result.is_err());
    }

    #[test]
    fn keystore_path_without_keystore_password() {
        let keystore_path = "./tests/test_data/keystore/test.json";

        let cmd = Command::parse_from(["sozo", "--keystore", keystore_path]);
        let result = cmd.signer.signer(None, true);

        assert!(result.is_err());
    }

    #[test]
    fn signer_without_pk_or_keystore() {
        let cmd = Command::parse_from(["sozo"]);
        let result = cmd.signer.signer(None, true);

        assert!(result.is_err());
    }
}
