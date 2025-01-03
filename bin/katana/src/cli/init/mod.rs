use std::fmt::Display;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use cainome::rs::abigen;
use clap::Args;
use dojo_utils::TransactionWaiter;
use inquire::{Confirm, CustomType};
use katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;
use katana_cairo::lang::starknet_classes::contract_class::ContractClass;
use katana_node::config::chain::{ChainConfig, SettlementLayer};
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::Genesis;
use katana_primitives::{felt, ContractAddress, Felt};
use lazy_static::lazy_static;
use starknet::accounts::{Account, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount};
use starknet::contract::ContractFactory;
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{BlockId, BlockTag, FlattenedSierraClass, StarknetError};
use starknet::core::utils::{cairo_short_string_to_felt, parse_cairo_short_string};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError, Url};
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

#[derive(Debug, Args)]
pub struct InitArgs {
    /// The path to where the config file will be written at.
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

        let output = ChainConfig {
            id: input.id,
            genesis: GENESIS.clone(),
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

        output.store(input.output_path)
    }

    fn prompt(&self, rt: &Runtime) -> Result<InitInput> {
        let chain_id = CustomType::<String>::new("Id")
            .with_help_message("This will be the id of your rollup chain.")
	        // checks that the input is a valid ascii string.
            .with_parser(&|input| {
                if input.is_ascii() {
                    Ok(input.to_string())
                } else {
                    Err(())
                }
            })
            .with_error_message("Must be valid ASCII characters")
            .prompt()?;

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

        // The core settlement contract on L1
        let settlement_contract =
    		// Prompt the user whether to deploy the settlement contract or not.
            if Confirm::new("Deploy settlement contract?").with_default(true).prompt()? {
                let result = rt.block_on(init_core_contract(&account));
                result.context("Failed to deploy settlement contract")?
            }
            // If denied, prompt the user for an already deployed contract.
            else {
	            // TODO: add a check to make sure the contract is indeed a valid settlement contract.
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

async fn init_core_contract<P>(
    account: &SingleOwnerAccount<P, LocalWallet>,
) -> Result<ContractAddress>
where
    P: Provider + Send + Sync,
{
    use spinoff::{spinners, Color, Spinner};

    let mut sp = Spinner::new(spinners::Dots, "", Color::Blue);

    let result = async {
        let class = include_str!(
            "../../../../../crates/katana/contracts/build/appchain_core_contract.json"
        );

        abigen!(
            AppchainContract,
            "[{\"type\":\"function\",\"name\":\"set_program_info\",\"inputs\":[{\"name\":\"\
             program_hash\",\"type\":\"core::felt252\"},{\"name\":\"config_hash\",\"type\":\"\
             core::felt252\"}],\"outputs\":[],\"state_mutability\":\"external\"}]"
        );

        let (contract, compiled_class_hash) = prepare_contract_declaration_params(class)?;
        let class_hash = contract.class_hash();

        // Check if the class has already been declared,
        match account.provider().get_class(BlockId::Tag(BlockTag::Pending), class_hash).await {
            Ok(..) => {
                // Class has already been declared, no need to do anything...
            }

            Err(ProviderError::StarknetError(StarknetError::ClassHashNotFound)) => {
                sp.update_text("Declaring contract...");
                let res = account.declare_v3(contract.into(), compiled_class_hash).send().await?;
                let _ = TransactionWaiter::new(res.transaction_hash, account.provider()).await?;
            }

            Err(err) => return Err(anyhow!(err)),
        }

        sp.update_text("Deploying contract...");

        let factory = ContractFactory::new(class_hash, &account);
        // appchain::constructor() https://github.com/cartridge-gg/piltover/blob/d373a844c3428383a48518adf468bf83249dec3a/src/appchain.cairo#L119-L125
        let request = factory.deploy_v1(
            vec![
                account.address(), // owner
                Felt::ZERO,        // state_root
                Felt::ZERO,        // block_number
                Felt::ZERO,        // block_hash
            ],
            Felt::ZERO,
            true,
        );

        let res = request.send().await?;
        let _ = TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

        sp.update_text("Initializing...");

        let deployed_contract_address = request.deployed_address();
        let appchain = AppchainContract::new(deployed_contract_address, account);

        const PROGRAM_HASH: Felt =
            felt!("0x5ab580b04e3532b6b18f81cfa654a05e29dd8e2352d88df1e765a84072db07");
        const CONFIG_HASH: Felt =
            felt!("0x504fa6e5eb930c0d8329d4a77d98391f2730dab8516600aeaf733a6123432");

        appchain.set_program_info(&PROGRAM_HASH, &CONFIG_HASH).send().await?;

        Ok(deployed_contract_address.into())
    }
    .await;

    match result {
        Ok(addr) => sp.success(&format!("Deployment successful ({addr})")),
        Err(..) => sp.fail("Deployment failed"),
    }
    result
}

fn prepare_contract_declaration_params(artifact: &str) -> Result<(FlattenedSierraClass, Felt)> {
    let class = get_flattened_class(artifact)?;
    let compiled_class_hash = get_compiled_class_hash(artifact)?;
    Ok((class, compiled_class_hash))
}

fn get_flattened_class(artifact: &str) -> Result<FlattenedSierraClass> {
    let contract_artifact: SierraClass = serde_json::from_str(artifact)?;
    Ok(contract_artifact.flatten()?)
}

fn get_compiled_class_hash(artifact: &str) -> Result<Felt> {
    let casm_contract_class: ContractClass = serde_json::from_str(artifact)?;
    let casm_contract =
        CasmContractClass::from_contract_class(casm_contract_class, true, usize::MAX)?;
    let res = serde_json::to_string(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}

// > CONFIG_DIR/$chain_id/config.json
fn config_path(id: &str) -> Result<PathBuf> {
    Ok(config_dir(id)?.join("config").with_extension("json"))
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

lazy_static! {
    static ref GENESIS: Genesis = {
        // master account
        let accounts = DevAllocationsGenerator::new(1).generate();
        let mut genesis = Genesis::default();
        genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
        genesis
    };
}
