use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::FieldElement;
use starknet::providers::Provider;
use starknet::signers::LocalWallet;

use super::signer::SignerOptions;
use super::DOJO_ACCOUNT_ADDRESS_ENV_VAR;

// INVARIANT:
// - For commandline: we can either specify `private_key` or `keystore_path` along with
//   `keystore_password`. This is enforced by Clap.
// - For `Scarb.toml`: if both private_key and keystore are specified in `Scarb.toml` private_key
//   will take priority
#[derive(Debug, Args)]
#[command(next_help_heading = "Account options")]
pub struct AccountOptions {
    #[arg(long, env = DOJO_ACCOUNT_ADDRESS_ENV_VAR)]
    #[arg(global = true)]
    pub account_address: Option<FieldElement>,

    #[command(flatten)]
    #[command(next_help_heading = "Signer options")]
    pub signer: SignerOptions,

    #[arg(long)]
    #[arg(help = "Use legacy account (cairo0 account)")]
    #[arg(global = true)]
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
        let signer = self.signer.signer(env_metadata, false)?;

        let chain_id =
            provider.chain_id().await.with_context(|| "Failed to retrieve network chain id.")?;

        let encoding = if self.legacy { ExecutionEncoding::Legacy } else { ExecutionEncoding::New };

        Ok(SingleOwnerAccount::new(provider, signer, account_address, chain_id, encoding))
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
    use clap::Parser;
    use starknet::accounts::{Call, ExecutionEncoder};
    use starknet_crypto::FieldElement;

    use super::{AccountOptions, DOJO_ACCOUNT_ADDRESS_ENV_VAR};

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

    #[katana_runner::katana_test(2, true)]
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

    #[katana_runner::katana_test(2, true)]
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
}
