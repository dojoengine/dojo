use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::{Args, ValueEnum};
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

    #[arg(long, global = true)]
    #[arg(value_enum)]
    #[arg(value_name = "NAME")]
    #[arg(help = "Use one of Katana's pre-funded dev accounts (katana0..katana9).")]
    pub katana_account: Option<KatanaAccount>,

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

        let katana_details = self.katana_details();
        let signer_options = if let Some(details) = katana_details {
            if !self.signer.has_custom_signer() {
                self.signer.with_private_key(details.private_key)
            } else {
                self.signer.clone()
            }
        } else {
            self.signer.clone()
        };

        let signer = signer_options.signer(env_metadata, false)?;

        trace!("Fetching chain id...");
        let chain_id = provider.chain_id().await?;
        trace!(?chain_id);

        let encoding = if self.legacy { ExecutionEncoding::Legacy } else { ExecutionEncoding::New };
        trace!(?encoding, "Creating SingleOwnerAccount.");
        let mut account =
            SingleOwnerAccount::new(provider, signer, account_address, chain_id, encoding);

        // Since now the block frequency is higher than before, using latest is
        // totally fine. We keep it explicitely set here to easy toggle if necessary.
        account.set_block_id(BlockId::Tag(BlockTag::Latest));
        Ok(account)
    }

    pub fn account_address(&self, env_metadata: Option<&Environment>) -> Result<Felt> {
        if let Some(address) = self.account_address {
            trace!(?address, "Account address found.");
            Ok(address)
        } else if let Some(details) = self.katana_details() {
            trace!(address = details.address, "Using Katana preset account address.");
            Ok(Felt::from_str(details.address)?)
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

    fn katana_details(&self) -> Option<KatanaAccountDetails> {
        self.katana_account.map(|preset| preset.details())
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

#[derive(Clone, Copy)]
struct KatanaAccountDetails {
    address: &'static str,
    private_key: &'static str,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum KatanaAccount {
    #[value(name = "katana0")]
    Katana0,
    #[value(name = "katana1")]
    Katana1,
    #[value(name = "katana2")]
    Katana2,
    #[value(name = "katana3")]
    Katana3,
    #[value(name = "katana4")]
    Katana4,
    #[value(name = "katana5")]
    Katana5,
    #[value(name = "katana6")]
    Katana6,
    #[value(name = "katana7")]
    Katana7,
    #[value(name = "katana8")]
    Katana8,
    #[value(name = "katana9")]
    Katana9,
}

impl KatanaAccount {
    fn details(self) -> KatanaAccountDetails {
        match self {
            KatanaAccount::Katana0 => KatanaAccountDetails {
                address: "0x127fd5f1fe78a71f8bcd1fec63e3fe2f0486b6ecd5c86a0466c3a21fa5cfcec",
                private_key: "0x00c5b2fcab997346f3ea1c00b002ecf6f382c5f9c9659a3894eb783c5320f912",
            },
            KatanaAccount::Katana1 => KatanaAccountDetails {
                address: "0x13d9ee239f33fea4f8785b9e3870ade909e20a9599ae7cd62c1c292b73af1b7",
                private_key: "0x01c9053c053edf324aec366a34c6901b1095b07af69495bffec7d7fe21effb1b",
            },
            KatanaAccount::Katana2 => KatanaAccountDetails {
                address: "0x17cc6ca902ed4e8baa8463a7009ff18cc294fa85a94b4ce6ac30a9ebd6057c7",
                private_key: "0x014d6672dcb4b77ca36a887e9a11cd9d637d5012468175829e9c6e770c61642",
            },
            KatanaAccount::Katana3 => KatanaAccountDetails {
                address: "0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba",
                private_key: "0x018000000003000001800000000000300000000000003006001800006600",
            },
            KatanaAccount::Katana4 => KatanaAccountDetails {
                address: "0x359b9068eadcaaa449c08b79a367c6fdfba9448c29e96934e3552dab0fdd950",
                private_key: "0x02bbf4f9fd0bbb2e60b0316c1fe0b76cf7a4d0198bd493ced9b8df2a3a24d68a",
            },
            KatanaAccount::Katana5 => KatanaAccountDetails {
                address: "0x4184158a64a82eb982ff702e4041a49db16fa3a18229aac4ce88c832baf56e4",
                private_key: "0x06bf3604bcb41fed6c42bcca5436eeb65083a982ff65db0dc123f65358008b51",
            },
            KatanaAccount::Katana6 => KatanaAccountDetails {
                address: "0x42b249d1633812d903f303d640a4261f58fead5aa24925a9efc1dd9d76fb555",
                private_key: "0x0283d1e73776cd4ac1ac5f0b879f561bded25eceb2cc589c674af0cec41df441",
            },
            KatanaAccount::Katana7 => KatanaAccountDetails {
                address: "0x4e0b838810cb1a355beb7b3d894ca0e98ee524309c3f8b7cccb15a48e6270e2",
                private_key: "0x0736adbbcdac7cc600f89051db1abbc16b9996b46f6b58a9752a11c1028a8ec8",
            },
            KatanaAccount::Katana8 => KatanaAccountDetails {
                address: "0x5b6b8189bb580f0df1e6d6bec509ff0d6c9be7365d10627e0cf222ec1b47a71",
                private_key: "0x0330030030018000099001803000d206308b0070db00121318d17b5e6262150b",
            },
            KatanaAccount::Katana9 => KatanaAccountDetails {
                address: "0x6677fe62ee39c7b07401f754138502bab7fac99d2d3c5d37df7d1c6fab10819",
                private_key: "0x03e3979c1ed728490308054fe357a9f49cf67f80f9721f44cc57235129e090f4",
            },
        }
    }
}
