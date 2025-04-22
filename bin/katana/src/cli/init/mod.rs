use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;
use clap::builder::NonEmptyStringValueParser;
use clap::Args;
use deployment::DeploymentOutcome;
use katana_chain_spec::rollup::{ChainConfigDir, FeeContract};
use katana_chain_spec::{rollup, SettlementLayer};
use katana_primitives::block::BlockNumber;
use katana_primitives::chain::ChainId;
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use katana_primitives::genesis::Genesis;
use katana_primitives::{ContractAddress, Felt, U256};
use settlement::SettlementChainProvider;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::utils::{cairo_short_string_to_felt, parse_cairo_short_string};
use starknet::providers::Provider;
use starknet::signers::SigningKey;
use url::Url;

mod deployment;
mod prompt;
mod settlement;
#[cfg(feature = "init-slot")]
mod slot;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// The id of the new chain to be initialized.
    ///
    /// An empty `Id` is not a allowed, since the chain id must be
    /// a valid ASCII string.
    #[arg(long)]
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    id: Option<String>,

    /// The settlement chain to be used, where the core contract is deployed.
    ///
    /// If a custom settlement chain is provided, setting a custom facts registry is required using
    /// the `--settlement-facts-registry` option. Otherwise, setting a custom facts registry
    /// with a known chain is a no-op.
    #[arg(long = "settlement-chain")]
    #[arg(required_unless_present = "sovereign")]
    #[arg(requires_all = ["id", "settlement_account", "settlement_account_private_key"])]
    settlement_chain: Option<SettlementChain>,

    /// The address of the settlement account to be used to configure the core contract.
    #[arg(long = "settlement-account-address")]
    #[arg(required_unless_present = "sovereign")]
    #[arg(requires_all = ["id", "settlement_chain", "settlement_account_private_key"])]
    settlement_account: Option<ContractAddress>,

    /// The private key of the settlement account to be used to configure the core contract.
    #[arg(long = "settlement-account-private-key")]
    #[arg(required_unless_present = "sovereign")]
    #[arg(requires_all = ["id", "settlement_chain", "settlement_account"])]
    settlement_account_private_key: Option<Felt>,

    /// The address of the settlement contract.
    /// If not provided, the contract will be deployed on the settlement chain using the provided
    /// settlement account.
    #[arg(long = "settlement-contract")]
    #[arg(requires_all = ["id", "settlement_chain", "settlement_account", "settlement_account_private_key", "settlement_contract_deployed_block"])]
    settlement_contract: Option<ContractAddress>,

    /// The block number of the settlement contract deployment.
    /// This value is required if the `settlement-contract` is provided, for Katana to
    /// know from which block the messages can be gathered from the settlement chain.
    #[arg(long = "settlement-contract-deployed-block")]
    #[arg(requires = "settlement_contract")]
    settlement_contract_deployed_block: Option<BlockNumber>,

    /// The address of the facts registry contract on the settlement chain.
    ///
    /// Required if a custom settlement chain is specified.
    #[arg(long = "settlement-facts-registry")]
    #[arg(requires_all = ["id", "settlement_chain", "settlement_account"])]
    pub settlement_facts_registry_contract: Option<ContractAddress>,

    /// Initialize a sovereign chain with no settlement layer, by only publishing the state updates
    /// and proofs on a Data Availability Layer. By using this flag, no settlement option is
    /// required.
    #[arg(long)]
    #[arg(help = "Initialize a sovereign chain with no settlement layer, by only publishing the \
                  state updates and proofs on a Data Availability Layer.")]
    #[arg(requires_all = ["id"])]
    #[arg(conflicts_with_all = ["settlement_chain", "settlement_account", "settlement_account_private_key", "settlement_contract"])]
    sovereign: bool,

    /// Specify the path of the directory where the configuration files will be stored at.
    #[arg(long)]
    output_path: Option<PathBuf>,

    #[cfg(feature = "init-slot")]
    #[command(flatten)]
    slot: slot::SlotArgs,
}

impl InitArgs {
    // TODO:
    // - deploy bridge contract
    pub(crate) async fn execute(self) -> anyhow::Result<()> {
        let output = if let Some(output) = self.configure_from_args().await {
            output?
        } else {
            prompt::prompt().await?
        };

        let settlement = match &output {
            AnyOutcome::Persistent(persistent) => SettlementLayer::Starknet {
                account: persistent.account,
                rpc_url: persistent.rpc_url.clone(),
                id: ChainId::parse(&persistent.settlement_id)?,
                block: persistent.deployment_outcome.block_number,
                core_contract: persistent.deployment_outcome.contract_address,
            },
            AnyOutcome::Sovereign(_) => SettlementLayer::Sovereign {},
        };

        let id = ChainId::parse(output.id())?;

        #[cfg_attr(not(feature = "init-slot"), allow(unused_mut))]
        let mut genesis = generate_genesis();
        #[cfg(feature = "init-slot")]
        slot::add_paymasters_to_genesis(
            &mut genesis,
            &output.slot_paymasters().unwrap_or_default(),
        );

        // At the moment, the fee token is limited to a predefined token.
        let fee_contract = FeeContract::default();
        let chain_spec = rollup::ChainSpec { id, genesis, settlement, fee_contract };

        if let Some(path) = self.output_path {
            let dir = ChainConfigDir::create(path)?;
            rollup::write(&dir, &chain_spec).context("failed to write chain spec file")?;
        } else {
            // Write to the local chain config directory by default if user
            // doesn't specify the output path
            rollup::write_local(&chain_spec).context("failed to write chain spec file")?;
        }

        Ok(())
    }

    async fn configure_from_args(&self) -> Option<anyhow::Result<AnyOutcome>> {
        if let Some(id) = self.id.clone() {
            if self.sovereign {
                return Some(Ok(AnyOutcome::Sovereign(SovereignOutcome {
                    id,
                    #[cfg(feature = "init-slot")]
                    slot_paymasters: self.slot.paymaster_accounts.clone(),
                })));
            }

            // These args are all required if at least one of them are specified (incl chain id) and
            // `clap` has already handled that for us, so it's safe to unwrap here.
            let settlement_chain = self.settlement_chain.clone().expect("must present");
            let settlement_account_address = self.settlement_account.expect("must present");
            let settlement_private_key = self.settlement_account_private_key.expect("must present");

            let settlement_provider = match settlement_chain {
                SettlementChain::Mainnet => {
                    let mut provider = SettlementChainProvider::sn_mainnet();
                    if let Some(fact_registry) = self.settlement_facts_registry_contract {
                        provider.set_fact_registry(*fact_registry);
                    }
                    provider
                }
                SettlementChain::Sepolia => {
                    let mut provider = SettlementChainProvider::sn_sepolia();
                    if let Some(fact_registry) = self.settlement_facts_registry_contract {
                        provider.set_fact_registry(*fact_registry);
                    }
                    provider
                }
                #[cfg(feature = "init-custom-settlement-chain")]
                SettlementChain::Custom(url) => {
                    let Some(fact_registry) = self.settlement_facts_registry_contract else {
                        return Some(Err(anyhow::anyhow!(
                            "Specifying the facts registry contract (using \
                             `--settlement-facts-registry`) is required when settling on a custom \
                             chain"
                        )));
                    };
                    SettlementChainProvider::new(url, *fact_registry)
                }
            };

            let l1_chain_id = settlement_provider.chain_id().await.unwrap();

            let chain_id = cairo_short_string_to_felt(&id).unwrap();

            let deployment_outcome = if let Some(contract) = self.settlement_contract {
                deployment::check_program_info(chain_id, contract.into(), &settlement_provider)
                    .await
                    .unwrap();

                DeploymentOutcome {
                    contract_address: contract,
                    block_number: self
                        .settlement_contract_deployed_block
                        .expect("must exist at this point"),
                }
            }
            // If settlement contract is not provided, then we will deploy it.
            else {
                let account = SingleOwnerAccount::new(
                    settlement_provider.clone(),
                    SigningKey::from_secret_scalar(settlement_private_key).into(),
                    settlement_account_address.into(),
                    l1_chain_id,
                    ExecutionEncoding::New,
                );

                deployment::deploy_settlement_contract(account, chain_id).await.unwrap()
            };

            Some(Ok(AnyOutcome::Persistent(PersistentOutcome {
                id,
                deployment_outcome,
                rpc_url: settlement_provider.url().clone(),
                account: settlement_account_address,
                settlement_id: parse_cairo_short_string(&l1_chain_id).unwrap(),
                #[cfg(feature = "init-slot")]
                slot_paymasters: self.slot.paymaster_accounts.clone(),
            })))
        } else {
            None
        }
    }
}

/// The outcome of the initialization process.
#[derive(Debug)]
enum AnyOutcome {
    Persistent(PersistentOutcome),
    Sovereign(SovereignOutcome),
}

impl AnyOutcome {
    pub fn id(&self) -> &str {
        match self {
            AnyOutcome::Persistent(persistent) => &persistent.id,
            AnyOutcome::Sovereign(sovereign) => &sovereign.id,
        }
    }

    #[cfg(feature = "init-slot")]
    pub fn slot_paymasters(&self) -> Option<Vec<slot::PaymasterAccountArgs>> {
        match self {
            AnyOutcome::Persistent(persistent) => persistent.slot_paymasters.clone(),
            AnyOutcome::Sovereign(sovereign) => sovereign.slot_paymasters.clone(),
        }
    }
}

#[derive(Debug)]
struct SovereignOutcome {
    /// The id of the new chain to be initialized.
    pub id: String,

    #[cfg(feature = "init-slot")]
    pub slot_paymasters: Option<Vec<slot::PaymasterAccountArgs>>,
}

#[derive(Debug)]
struct PersistentOutcome {
    /// the account address that is used to send the transactions for contract
    /// deployment/initialization.
    pub account: ContractAddress,

    // the id of the new chain to be initialized.
    pub id: String,

    // the chain id of the settlement layer.
    pub settlement_id: String,

    // the rpc url for the settlement layer.
    pub rpc_url: Url,

    pub deployment_outcome: DeploymentOutcome,

    #[cfg(feature = "init-slot")]
    pub slot_paymasters: Option<Vec<slot::PaymasterAccountArgs>>,
}

fn generate_genesis() -> Genesis {
    let accounts = DevAllocationsGenerator::new(1)
        .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
        .generate();
    let mut genesis = Genesis::default();
    genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
    genesis
}

#[derive(Debug, thiserror::Error)]
#[error("Unsupported settlement chain: {id}")]
struct SettlementChainTryFromStrError {
    id: String,
}

#[derive(Debug, Clone, strum_macros::Display, PartialEq, Eq)]
enum SettlementChain {
    Mainnet,
    Sepolia,
    #[cfg(feature = "init-custom-settlement-chain")]
    Custom(Url),
}

impl std::str::FromStr for SettlementChain {
    type Err = SettlementChainTryFromStrError;
    fn from_str(s: &str) -> Result<SettlementChain, <Self as ::core::str::FromStr>::Err> {
        let id = s.to_lowercase();
        if &id == "sepolia" || &id == "sn_sepolia" {
            return Ok(SettlementChain::Sepolia);
        }

        if &id == "mainnet" || &id == "sn_mainnet" {
            return Ok(SettlementChain::Mainnet);
        }

        #[cfg(feature = "init-custom-settlement-chain")]
        if let Ok(url) = Url::parse(s) {
            return Ok(SettlementChain::Custom(url));
        };

        Err(SettlementChainTryFromStrError { id: s.to_string() })
    }
}

impl TryFrom<&str> for SettlementChain {
    type Error = SettlementChainTryFromStrError;
    fn try_from(s: &str) -> Result<SettlementChain, <Self as TryFrom<&str>>::Error> {
        SettlementChain::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use clap::error::{ContextKind, ContextValue};
    use clap::Parser;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("sepolia", SettlementChain::Sepolia)]
    #[case("SEPOLIA", SettlementChain::Sepolia)]
    #[case("sn_sepolia", SettlementChain::Sepolia)]
    #[case("SN_SEPOLIA", SettlementChain::Sepolia)]
    #[case("mainnet", SettlementChain::Mainnet)]
    #[case("MAINNET", SettlementChain::Mainnet)]
    #[case("sn_mainnet", SettlementChain::Mainnet)]
    #[case("SN_MAINNET", SettlementChain::Mainnet)]
    fn test_chain_from_str(#[case] input: &str, #[case] expected: SettlementChain) {
        assert_matches!(SettlementChain::from_str(input), Ok(chain) if chain == expected);
    }

    #[test]
    fn invalid_chain() {
        assert!(SettlementChain::from_str("invalid_chain").is_err());
    }

    #[test]
    #[cfg(feature = "init-custom-settlement-chain")]
    fn custom_settlement_chain() {
        assert_matches!(
            SettlementChain::from_str("http://localhost:5050"),
            Ok(SettlementChain::Custom(actual_url)) => {
                assert_eq!(actual_url, Url::parse("http://localhost:5050").unwrap());
            }
        );
    }

    #[test]
    fn non_sovereign_requires_all_settlement_args() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: InitArgs,
        }

        // This should fail with the expected error message:-
        //
        // ```
        // error: the following required arguments were not provided:
        //   --settlement-chain <SETTLEMENT_CHAIN>
        //   --settlement-account-address <SETTLEMENT_ACCOUNT>
        //   --settlement-account-private-key <SETTLEMENT_ACCOUNT_PRIVATE_KEY>
        // ```
        match Cli::try_parse_from(["init", "--id", "bruh"]) {
            Ok(..) => panic!("Expected parsing to fail with missing required arguments"),
            Err(err) => {
                if let ContextValue::Strings(values) = err.get(ContextKind::InvalidArg).unwrap() {
                    // Assert that the error message contains all the required arguments
                    assert!(values.contains(&"--settlement-chain <SETTLEMENT_CHAIN>".to_string()));
                    assert!(values.contains(
                        &"--settlement-account-address <SETTLEMENT_ACCOUNT>".to_string()
                    ));
                    assert!(
                        values.contains(
                            &"--settlement-account-private-key <SETTLEMENT_ACCOUNT_PRIVATE_KEY>"
                                .to_string()
                        )
                    );
                } else {
                    panic!("Expected InvalidArg context with Strings value");
                }
            }
        }
    }

    #[test]
    fn sovereign_does_not_require_settlement_args() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: InitArgs,
        }

        Cli::parse_from(["init", "--id", "bruh", "--sovereign"]);
    }

    #[test]
    fn cli_accept_custom_fact_registry() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: InitArgs,
        }

        let custom_settlement_fact_registry = "0x1234567890123456789012345678901234567890";
        let result = Cli::parse_from([
            "init",
            "--id",
            "wot",
            "--settlement-chain",
            "sepolia",
            "--settlement-account-address",
            "0x1234567890123456789012345678901234567890",
            "--settlement-account-private-key",
            "0x1234567890123456789012345678901234567890",
            "--settlement-facts-registry",
            custom_settlement_fact_registry,
        ]);
        assert_eq!(
            result.args.settlement_facts_registry_contract,
            Some(ContractAddress::from_str(custom_settlement_fact_registry).unwrap())
        );
    }

    #[test]
    fn cli_required_settlement_args_with_custom_fact_registry() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: InitArgs,
        }

        // This should fail with the expected error message:-
        //
        // ```
        // error: the following required arguments were not provided:
        //   --settlement-chain <SETTLEMENT_CHAIN>
        //   --settlement-account-address <SETTLEMENT_ACCOUNT>
        //   --settlement-account-private-key <SETTLEMENT_ACCOUNT_PRIVATE_KEY>
        // ```
        match Cli::try_parse_from([
            "init",
            "--id",
            "wot",
            "--settlement-facts-registry",
            "0x1234567890123456789012345678901234567890",
        ]) {
            Ok(..) => panic!("Expected parsing to fail with missing required arguments"),
            Err(err) => {
                if let ContextValue::Strings(values) = err.get(ContextKind::InvalidArg).unwrap() {
                    // Assert that the error message contains all the required arguments
                    assert!(values.contains(&"--settlement-chain <SETTLEMENT_CHAIN>".to_string()));
                    assert!(values.contains(
                        &"--settlement-account-address <SETTLEMENT_ACCOUNT>".to_string()
                    ));
                    assert!(
                        values.contains(
                            &"--settlement-account-private-key <SETTLEMENT_ACCOUNT_PRIVATE_KEY>"
                                .to_string()
                        )
                    );
                } else {
                    panic!("Expected InvalidArg context with Strings value");
                }
            }
        }
    }

    #[tokio::test]
    async fn cli_required_custom_fact_registry_for_custom_init_chain() {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            args: InitArgs,
        }

        let result = Cli::parse_from([
            "init",
            "--id",
            "wot",
            "--settlement-chain",
            "http://localhost:5050",
            "--settlement-account-address",
            "0x1234567890123456789012345678901234567890",
            "--settlement-account-private-key",
            "0x1234567890123456789012345678901234567890",
        ]);
        assert_eq!(result.args.settlement_facts_registry_contract, None);

        let configure_result = result.args.configure_from_args().await;
        assert!(configure_result.is_some());
        let configure_result = configure_result.unwrap();
        assert!(configure_result.is_err());
        assert_eq!(
            configure_result.unwrap_err().to_string(),
            "Specifying the facts registry contract (using `--settlement-facts-registry`) is \
             required when settling on a custom chain"
        );
    }
}
