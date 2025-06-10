use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use clap::Args;
use dojo_utils::env::DOJO_ACCOUNT_ADDRESS_ENV_VAR;
use dojo_world::config::Environment;
use dojo_world::contracts::ContractInfo;
#[cfg(feature = "controller")]
use slot::account_sdk::provider::CartridgeJsonRpcProvider;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, Felt};
use starknet::providers::Provider;
use starknet::signers::LocalWallet;
use tracing::trace;

use super::signer::SignerOptions;
use super::starknet::StarknetOptions;

#[cfg(feature = "controller")]
pub mod controller;
pub mod provider;
mod r#type;

#[cfg(feature = "controller")]
use controller::ControllerAccount;
pub use r#type::*;

// INVARIANT:
// - For commandline: we can either specify `private_key` or `keystore_path` along with
//   `keystore_password`. This is enforced by Clap.
// - For `Scarb.toml`: if both private_key and keystore are specified in `Scarb.toml` private_key
//   will take priority
#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Account options")]
pub struct AccountOptions {
    #[arg(long, env = DOJO_ACCOUNT_ADDRESS_ENV_VAR)]
    #[arg(global = true)]
    pub account_address: Option<Felt>,

    #[arg(global = true)]
    #[arg(long = "slot.controller")]
    #[arg(help_heading = "Controller options")]
    #[arg(help = "Use Slot's Controller account")]
    #[cfg(feature = "controller")]
    pub controller: bool,

    #[command(flatten)]
    #[command(next_help_heading = "Signer options")]
    pub signer: SignerOptions,

    #[arg(long)]
    #[arg(help = "Use legacy account (cairo0 account)")]
    #[arg(global = true)]
    pub legacy: bool,
}

impl AccountOptions {
    /// Creates a [`SozoAccount`] from the given parameters.
    ///
    /// # Arguments
    ///
    /// * `provider` - Starknet provider (only if you're NOT creating a Controller account).
    /// * `env_metadata` - Environment pulled from configuration.
    /// * `starknet` - Starknet options.
    /// * `contracts` - The [`ContractInfo`] mappings. This one could have been gated behind the
    ///   controller feature. However, to keep the feature internalized to account option, it's not.
    ///   The caller could easily provide a default value though.
    pub async fn account<P>(
        &self,
        provider: P,
        env_metadata: Option<&Environment>,
        starknet: &StarknetOptions,
        contracts: &HashMap<String, ContractInfo>,
    ) -> Result<SozoAccount<P>>
    where
        P: Provider,
        P: Send + Sync,
    {
        #[cfg(feature = "controller")]
        if self.controller {
            let url = starknet.url(env_metadata)?;
            let cartridge_provider = CartridgeJsonRpcProvider::new(url.clone());
            let account = self.controller(url, cartridge_provider.clone(), contracts).await?;
            return Ok(SozoAccount::new_controller(cartridge_provider, account));
        }

        let _ = starknet;
        let _ = contracts;

        let provider = Arc::new(provider);
        let account = self.std_account(provider.clone(), env_metadata).await?;
        Ok(SozoAccount::new_standard(provider, account))
    }

    /// Create a new Catridge Controller account based on session key.
    #[cfg(feature = "controller")]
    pub async fn controller(
        &self,
        rpc_url: url::Url,
        rpc_provider: CartridgeJsonRpcProvider,
        contracts: &HashMap<String, ContractInfo>,
    ) -> Result<ControllerAccount> {
        use anyhow::Context;
        controller::create_controller(rpc_url, rpc_provider, contracts)
            .await
            .context("Failed to create a Controller account")
    }

    pub async fn std_account<P>(
        &self,
        provider: Arc<P>,
        env_metadata: Option<&Environment>,
    ) -> Result<SingleOwnerAccount<Arc<P>, LocalWallet>>
    where
        P: Provider,
        P: Send + Sync,
    {
        let account_address = self.account_address(env_metadata)?;

        let signer = self.signer.signer(env_metadata, false)?;

        trace!("Fetching chain id...");
        let chain_id = provider.chain_id().await?;
        trace!(?chain_id);

        let encoding = if self.legacy { ExecutionEncoding::Legacy } else { ExecutionEncoding::New };
        trace!(?encoding, "Creating SingleOwnerAccount.");
        let mut account =
            SingleOwnerAccount::new(provider, signer, account_address, chain_id, encoding);

        // The default is `Latest` in starknet-rs, which does not reflect
        // the nonce changes in the pending block.
        account.set_block_id(BlockId::Tag(BlockTag::Pending));
        Ok(account)
    }

    pub fn account_address(&self, env_metadata: Option<&Environment>) -> Result<Felt> {
        if let Some(address) = self.account_address {
            trace!(?address, "Account address found.");
            Ok(address)
        } else if let Some(address) = env_metadata.and_then(|env| env.account_address()) {
            trace!(address, "Account address found in environment metadata.");
            Ok(Felt::from_str(address)?)
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
    use std::sync::Arc;

    use clap::Parser;
    use katana_runner::RunnerCtx;
    use starknet::accounts::ExecutionEncoder;
    use starknet::core::types::Call;
    use starknet_crypto::Felt;

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
        assert_eq!(cmd.account.account_address, Some(Felt::from_hex("0x0").unwrap()));
    }

    #[test]
    fn account_address_from_args() {
        let cmd = Command::parse_from(["sozo", "--account-address", "0x0"]);
        assert_eq!(cmd.account.account_address(None).unwrap(), Felt::from_hex("0x0").unwrap());
    }

    #[test]
    fn account_address_from_env_metadata() {
        let env_metadata = dojo_world::config::Environment {
            account_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);
        assert_eq!(
            cmd.account.account_address(Some(&env_metadata)).unwrap(),
            Felt::from_hex("0x0").unwrap()
        );
    }

    #[test]
    fn account_address_from_both() {
        let env_metadata = dojo_world::config::Environment {
            account_address: Some("0x0".to_owned()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--account-address", "0x1"]);
        assert_eq!(
            cmd.account.account_address(Some(&env_metadata)).unwrap(),
            Felt::from_hex("0x1").unwrap()
        );
    }

    #[test]
    fn account_address_from_neither() {
        let cmd = Command::parse_from([""]);
        assert!(cmd.account.account_address(None).is_err());
    }

    #[tokio::test]
    #[katana_runner::test(accounts = 2, fee = false)]
    async fn legacy_flag_works_as_expected(runner: &RunnerCtx) {
        let cmd = Command::parse_from([
            "sozo",
            "--legacy",
            "--account-address",
            "0x0",
            "--private-key",
            "0x1",
        ]);
        let dummy_call = vec![Call {
            to: Felt::from_hex("0x0").unwrap(),
            selector: Felt::from_hex("0x1").unwrap(),
            calldata: vec![Felt::from_hex("0x2").unwrap(), Felt::from_hex("0x3").unwrap()],
        }];

        // HACK: SingleOwnerAccount doesn't expose a way to check `encoding` type used in struct, so
        // checking it by encoding a dummy call and checking which method it used to encode the call
        let account = cmd.account.std_account(Arc::new(runner.provider()), None).await.unwrap();
        let result = account.encode_calls(&dummy_call);
        // 0x0 is the data offset.
        assert!(*result.get(3).unwrap() == Felt::from_hex("0x0").unwrap());
    }

    #[tokio::test]
    #[katana_runner::test(accounts = 2, fee = false)]
    async fn without_legacy_flag_works_as_expected(runner: &RunnerCtx) {
        let cmd = Command::parse_from(["sozo", "--account-address", "0x0", "--private-key", "0x1"]);
        let dummy_call = vec![Call {
            to: Felt::from_hex("0x0").unwrap(),
            selector: Felt::from_hex("0x1").unwrap(),
            calldata: vec![Felt::from_hex("0xf2").unwrap(), Felt::from_hex("0xf3").unwrap()],
        }];

        // HACK: SingleOwnerAccount doesn't expose a way to check `encoding` type used in struct, so
        // checking it by encoding a dummy call and checking which method it used to encode the call
        let account = cmd.account.std_account(Arc::new(runner.provider()), None).await.unwrap();
        let result = account.encode_calls(&dummy_call);
        // 0x2 is the Calldata len.
        assert!(*result.get(3).unwrap() == Felt::from_hex("0x2").unwrap());
    }
}
