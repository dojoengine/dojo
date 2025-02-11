use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use clap::Args;
use katana_chain_spec::rollup::{ChainConfigDir, FeeContract};
use katana_chain_spec::{rollup, SettlementLayer};
use katana_primitives::chain::ChainId;
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use katana_primitives::genesis::Genesis;
use katana_primitives::{ContractAddress, Felt, U256};
use lazy_static::lazy_static;
use prompt::CARTRIDGE_SN_SEPOLIA_PROVIDER;
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::utils::{cairo_short_string_to_felt, parse_cairo_short_string};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet::signers::SigningKey;
use url::Url;

mod deployment;
mod prompt;

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long)]
    #[arg(requires_all = ["settlement_chain", "settlement_account", "settlement_account_private_key"])]
    id: Option<String>,

    #[arg(long = "settlement-chain")]
    #[arg(requires_all = ["id", "settlement_account", "settlement_account_private_key"])]
    settlement_chain: Option<SettlementChain>,

    #[arg(long = "settlement-account-address")]
    #[arg(requires_all = ["id", "settlement_chain", "settlement_account_private_key"])]
    settlement_account: Option<ContractAddress>,

    #[arg(long = "settlement-account-private-key")]
    #[arg(requires_all = ["id", "settlement_chain", "settlement_account"])]
    settlement_account_private_key: Option<Felt>,

    #[arg(long = "settlement-contract")]
    #[arg(requires_all = ["id", "settlement_chain", "settlement_account", "settlement_account_private_key"])]
    settlement_contract: Option<ContractAddress>,

    /// Specify the path of the directory where the configuration files will be stored at.
    #[arg(long)]
    output_path: Option<PathBuf>,
}

impl InitArgs {
    // TODO:
    // - deploy bridge contract
    pub(crate) async fn execute(self) -> anyhow::Result<()> {
        let output = if let Some(output) = self.process_args().await {
            output?
        } else {
            prompt::prompt().await?
        };

        let settlement = SettlementLayer::Starknet {
            account: output.account,
            rpc_url: output.rpc_url,
            id: ChainId::parse(&output.settlement_id)?,
            core_contract: output.settlement_contract,
        };

        let id = ChainId::parse(&output.id)?;
        let genesis = GENESIS.clone();
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

    async fn process_args(&self) -> Option<anyhow::Result<Outcome>> {
        // Here we just check that if `id` is present, then all the other required* arguments must
        // be present as well. This is guaranteed by `clap`.
        if let Some(id) = self.id.clone() {
            // These args are all required if at least one of them are specified (incl chain id) and
            // `clap` has already handled that for us, so it's safe to unwrap here.
            let settlement_chain = self.settlement_chain.clone().expect("must present");
            let settlement_account_address = self.settlement_account.expect("must present");
            let settlement_private_key = self.settlement_account_private_key.expect("must present");

            let settlement_url = match settlement_chain {
                SettlementChain::Sepolia => Url::parse(CARTRIDGE_SN_SEPOLIA_PROVIDER).unwrap(),
                #[cfg(feature = "init-custom-settlement-chain")]
                SettlementChain::Custom(url) => url,
            };

            let l1_provider =
                Arc::new(JsonRpcClient::new(HttpTransport::new(settlement_url.clone())));
            let l1_chain_id = l1_provider.chain_id().await.unwrap();

            let settlement_contract = if let Some(contract) = self.settlement_contract {
                let chain_id = cairo_short_string_to_felt(&id).unwrap();
                deployment::check_program_info(chain_id, contract.into(), &l1_provider)
                    .await
                    .unwrap();
                contract
            }
            // If settlement contract is not provided, then we will deploy it.
            else {
                let account = SingleOwnerAccount::new(
                    l1_provider,
                    SigningKey::from_secret_scalar(settlement_private_key).into(),
                    settlement_account_address.into(),
                    l1_chain_id,
                    ExecutionEncoding::New,
                );

                deployment::deploy_settlement_contract(account, l1_chain_id).await.unwrap()
            };

            Some(Ok(Outcome {
                id,
                settlement_contract,
                rpc_url: settlement_url,
                account: settlement_account_address,
                settlement_id: parse_cairo_short_string(&l1_chain_id).unwrap(),
            }))
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct Outcome {
    /// the account address that is used to send the transactions for contract
    /// deployment/initialization.
    pub account: ContractAddress,

    // the id of the new chain to be initialized.
    pub id: String,

    // the chain id of the settlement layer.
    pub settlement_id: String,

    // the rpc url for the settlement layer.
    pub rpc_url: Url,

    pub settlement_contract: ContractAddress,
}

lazy_static! {
    static ref GENESIS: Genesis = {
        // master account
        let accounts = DevAllocationsGenerator::new(1).with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE)).generate();
        let mut genesis = Genesis::default();
        genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
        genesis
    };
}

#[derive(Debug, thiserror::Error)]
#[error("Unsupported settlement chain: {id}")]
struct SettlementChainTryFromStrError {
    id: String,
}

#[derive(Debug, Clone, strum_macros::Display)]
enum SettlementChain {
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

    use super::*;

    #[test]
    fn sepolia_from_str() {
        assert_matches!(SettlementChain::from_str("sepolia"), Ok(SettlementChain::Sepolia));
        assert_matches!(SettlementChain::from_str("SEPOLIA"), Ok(SettlementChain::Sepolia));
        assert_matches!(SettlementChain::from_str("sn_sepolia"), Ok(SettlementChain::Sepolia));
        assert_matches!(SettlementChain::from_str("SN_SEPOLIA"), Ok(SettlementChain::Sepolia));
    }

    #[test]
    fn invalid_chain() {
        assert!(SettlementChain::from_str("invalid_chain").is_err());
    }

    #[test]
    fn try_from_str() {
        assert!(matches!(SettlementChain::try_from("sepolia"), Ok(SettlementChain::Sepolia)));
        assert!(SettlementChain::try_from("invalid").is_err(),);
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
}
