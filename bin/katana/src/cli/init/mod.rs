mod deployment;

use std::fmt::Display;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use inquire::{Confirm, CustomType, Text};
use katana_primitives::{ContractAddress, Felt};
use serde::{Deserialize, Serialize};
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::{cairo_short_string_to_felt, parse_cairo_short_string};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, Url};
use starknet::signers::{LocalWallet, SigningKey};
use tokio::runtime::Runtime;

#[derive(Debug)]
struct InitInput {
    /// the account address that is used to send the transactions for contract
    /// deployment/initialization.
    account: ContractAddress,

    // the id of the new chain to be initialized.
    id: String,

    // the chain id of the settlement layer.
    settlement_id: String,

    // the rpc url for the settlement layer.
    rpc_url: Url,

    fee_token: ContractAddress,

    settlement_contract: ContractAddress,

    // path at which the config file will be written at.
    output_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SettlementLayer {
    // the account address that was used to initialized the l1 deployments
    pub account: ContractAddress,

    // The id of the settlement chain.
    pub id: String,

    pub rpc_url: Url,

    // - The token that will be used to pay for tx fee in the appchain.
    // - For now, this must be the native token that is used to pay for tx fee in the settlement
    //   chain.
    pub fee_token: ContractAddress,

    // - The bridge contract for bridging the fee token from L1 to the appchain
    // - This will be part of the initialization process.
    pub bridge_contract: ContractAddress,

    // - The core appchain contract used to settlement
    // - This is deployed on the L1
    pub settlement_contract: ContractAddress,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InitConfiguration {
    // the initialized chain id
    pub id: String,

    // the fee token contract
    //
    // this corresponds to the l1 token contract
    pub fee_token: ContractAddress,

    pub settlement: SettlementLayer,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(value_name = "PATH")]
    pub output_path: Option<PathBuf>,
}

impl InitArgs {
    // TODO:
    // - deploy bridge contract
    // - generate the genesis
    pub(crate) fn execute(self) -> Result<()> {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
        let input = self.prompt(&rt)?;

        let output = InitConfiguration {
            id: input.id,
            fee_token: ContractAddress::default(),
            settlement: SettlementLayer {
                account: input.account,
                id: input.settlement_id,
                rpc_url: input.rpc_url,
                fee_token: input.fee_token,
                bridge_contract: ContractAddress::default(),
                settlement_contract: input.settlement_contract,
            },
        };

        let content = toml::to_string_pretty(&output)?;
        fs::write(input.output_path, content)?;

        Ok(())
    }

    fn prompt(&self, rt: &Runtime) -> Result<InitInput> {
        let chain_id = Text::new("Id").prompt()?;

        let url = CustomType::<Url>::new("Settlement RPC URL")
            .with_default(Url::parse("http://localhost:5050")?)
            .with_error_message("Please enter a valid URL")
            .prompt()?;

        let l1_provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url.clone())));

        let contract_exist_parser = &|input: &str| {
            let block_id = BlockId::Tag(BlockTag::Pending);
            let address = Felt::from_str(input).map_err(|_| ())?;
            let result = rt.block_on(l1_provider.clone().get_class_hash_at(block_id, address));

            match result {
                Ok(..) => Ok(ContractAddress::from(address)),
                Err(..) => Err(()),
            }
        };

        let account_address = CustomType::<ContractAddress>::new("Account")
            .with_error_message("Please enter a valid account address")
            .with_parser(contract_exist_parser)
            .prompt()?;

        let private_key = CustomType::<Felt>::new("Private key")
            .with_formatter(&|input: Felt| format!("{input:#x}"))
            .prompt()?;

        let l1_chain_id = rt.block_on(l1_provider.chain_id())?;
        let account = SingleOwnerAccount::new(
            l1_provider.clone(),
            LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private_key)),
            account_address.into(),
            l1_chain_id,
            ExecutionEncoding::New,
        );

        // The L1 fee token. Must be an existing token.
        let fee_token = CustomType::<ContractAddress>::new("Fee token")
            .with_parser(contract_exist_parser)
            .with_error_message("Please enter a valid fee token (the token must exist on L1)")
            .prompt()?;

        // The core settlement contract on L1c.
        // Prompt the user whether to deploy the settlement contract or not.
        let settlement_contract =
            if Confirm::new("Deploy settlement contract?").with_default(true).prompt()? {
                let chain_id = cairo_short_string_to_felt(&chain_id)?;
                let initialize = deployment::deploy_settlement_contract(account, chain_id);
                let result = rt.block_on(initialize);
                result?
            }
            // If denied, prompt the user for an already deployed contract.
            else {
                // TODO: add a check to make sure the contract is indeed a valid settlement
                // contract.
                CustomType::<ContractAddress>::new("Settlement contract")
                    .with_parser(contract_exist_parser)
                    .prompt()?
            };

        let output_path = if let Some(path) = self.output_path.clone() {
            path
        } else {
            CustomType::<Path>::new("Output path")
                .with_default(config_path(&chain_id).map(Path)?)
                .prompt()?
                .0
        };

        Ok(InitInput {
            account: account_address,
            settlement_contract,
            settlement_id: parse_cairo_short_string(&l1_chain_id)?,
            id: chain_id,
            fee_token,
            rpc_url: url,
            output_path,
        })
    }
}

// > CONFIG_DIR/$chain_id/config.toml
fn config_path(id: &str) -> Result<PathBuf> {
    Ok(config_dir(id)?.join("config").with_extension("toml"))
}

fn config_dir(id: &str) -> Result<PathBuf> {
    const KATANA_DIR: &str = "katana";

    let _ = cairo_short_string_to_felt(id).context("Invalid id");
    let path = dirs::config_local_dir().context("unsupported OS")?.join(KATANA_DIR).join(id);

    if !path.exists() {
        fs::create_dir_all(&path).expect("failed to create config directory");
    }

    Ok(path)
}

#[derive(Debug, Clone)]
struct Path(PathBuf);

impl FromStr for Path {
    type Err = <PathBuf as FromStr>::Err;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        PathBuf::from_str(s).map(Self)
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
