use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use dojo_utils::env::{
    DOJO_KEYSTORE_PASSWORD_ENV_VAR, DOJO_KEYSTORE_PATH_ENV_VAR, DOJO_PRIVATE_KEY_ENV_VAR,
};
use dojo_utils::keystore::prompt_password_if_needed;
use dojo_world::config::Environment;
use resolve_path::PathResolveExt;
use starknet::core::types::Felt;
use starknet::signers::{LocalWallet, SigningKey};
use tracing::trace;

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
    pub fn with_private_key(&self, private_key: &str) -> Self {
        let mut cloned = self.clone();
        cloned.private_key = Some(private_key.to_owned());
        cloned.keystore_path = None;
        cloned.keystore_password = None;
        cloned
    }

    pub fn has_custom_signer(&self) -> bool {
        self.private_key.is_some() || self.keystore_path.is_some()
    }

    /// Retrieves the signer from the CLI or environment metadata.
    /// First, attempt to locate the signer from CLI arguments or environment variables via CLAP.
    /// If unsuccessful, then search for the signer within the Dojo environment metadata.
    /// If the signer is not found in any of the above locations, return an error.
    pub fn signer(&self, env_metadata: Option<&Environment>, no_wait: bool) -> Result<LocalWallet> {
        let pk_cli = self.private_key.clone();
        let pk_env = env_metadata.and_then(|env| env.private_key().map(|s| s.to_string()));

        let pk_keystore_cli = self.private_key_from_keystore_cli(env_metadata, no_wait)?;
        let pk_keystore_env = self.private_key_from_keystore_env(env_metadata, no_wait)?;

        let private_key = if let Some(private_key) = pk_cli {
            trace!("Signing using private key from CLI.");
            SigningKey::from_secret_scalar(Felt::from_str(&private_key)?)
        } else if let Some(private_key) = pk_keystore_cli {
            trace!("Signing using private key from CLI keystore.");
            private_key
        } else if let Some(private_key) = pk_env {
            trace!("Signing using private key from env metadata.");
            SigningKey::from_secret_scalar(Felt::from_str(&private_key)?)
        } else if let Some(private_key) = pk_keystore_env {
            trace!("Signing using private key from env metadata keystore.");
            private_key
        } else {
            return Err(anyhow!(
                "Could not find private key. Please specify the private key or path to the \
                 keystore file."
            ));
        };

        Ok(LocalWallet::from_signing_key(private_key))
    }

    /// Retrieves the private key from the CLI keystore.
    /// If the keystore path is not set, it returns `None`.
    pub fn private_key_from_keystore_cli(
        &self,
        env_metadata: Option<&Environment>,
        no_wait: bool,
    ) -> Result<Option<SigningKey>> {
        if let Some(path) = &self.keystore_path {
            let maybe_password = if self.keystore_password.is_some() {
                self.keystore_password.as_deref()
            } else {
                env_metadata.and_then(|env| env.keystore_password())
            };

            let password = prompt_password_if_needed(maybe_password, no_wait)?;

            let private_key = SigningKey::from_keystore(Path::new(path).resolve(), &password)?;
            return Ok(Some(private_key));
        }

        Ok(None)
    }

    /// Retrieves the private key from the keystore in the environment metadata.
    /// If the keystore path is not set, it returns `None`.
    pub fn private_key_from_keystore_env(
        &self,
        env_metadata: Option<&Environment>,
        no_wait: bool,
    ) -> Result<Option<SigningKey>> {
        if let Some(path) = env_metadata.and_then(|env| env.keystore_path()) {
            let maybe_password = if self.keystore_password.is_some() {
                self.keystore_password.as_deref()
            } else {
                env_metadata.and_then(|env| env.keystore_password())
            };

            let password = prompt_password_if_needed(maybe_password, no_wait)?;

            let private_key = SigningKey::from_keystore(Path::new(path).resolve(), &password)?;
            return Ok(Some(private_key));
        }

        Ok(None)
    }

    /// Retrieves the private key from the CLI or environment metadata.
    pub fn _private_key(&self, env_metadata: Option<&Environment>) -> Option<String> {
        if let Some(s) = &self.private_key {
            Some(s.to_owned())
        } else {
            env_metadata.and_then(|env| env.private_key().map(|s| s.to_string()))
        }
    }

    /// Retrieves the keystore path from the CLI or environment metadata.
    pub fn _keystore_path(&self, env_metadata: Option<&Environment>) -> Option<String> {
        if let Some(s) = &self.keystore_path {
            Some(s.to_owned())
        } else {
            env_metadata.and_then(|env| env.keystore_path().map(|s| s.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use clap::Parser;
    use starknet::signers::{LocalWallet, Signer, SigningKey};
    use starknet_crypto::Felt;

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
            Felt::from_str(private_key).unwrap(),
        ));

        let result_public_key = result_wallet.get_public_key().await.unwrap();
        let expected_public_key = expected_wallet.get_public_key().await.unwrap();
        assert!(result_public_key.scalar() == expected_public_key.scalar());
    }

    #[tokio::test]
    async fn private_key_from_env_metadata() {
        let private_key = "0x1";
        let env_metadata = dojo_world::config::Environment {
            private_key: Some(private_key.to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo"]);
        let result_wallet = cmd.signer.signer(Some(&env_metadata), true).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            Felt::from_str(private_key).unwrap(),
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
            Felt::from_str(private_key).unwrap(),
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
        let env_metadata = dojo_world::config::Environment {
            keystore_path: Some(keystore_path.to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--password", keystore_password]);
        let result_wallet = cmd.signer.signer(Some(&env_metadata), true).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            Felt::from_str(private_key).unwrap(),
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

        let env_metadata = dojo_world::config::Environment {
            keystore_password: Some(keystore_password.to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--keystore", keystore_path]);
        let result_wallet = cmd.signer.signer(Some(&env_metadata), true).unwrap();
        let expected_wallet = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            Felt::from_str(private_key).unwrap(),
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
