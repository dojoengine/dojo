use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use dojo_utils::TransactionWaiter;
use inquire::parser::CustomTypeParser;
use inquire::validator::{CustomTypeValidator, StringValidator, Validation};
use inquire::{CustomType, CustomUserError, Text};
use katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;
use katana_cairo::lang::starknet_classes::contract_class::ContractClass;
use katana_primitives::{ContractAddress, Felt};
use serde::{Deserialize, Serialize};
use starknet::accounts::{Account, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount};
use starknet::contract::ContractFactory;
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{BlockId, BlockTag, FlattenedSierraClass};
use starknet::core::utils::{cairo_short_string_to_felt, parse_cairo_short_string};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, Url};
use starknet::signers::{LocalWallet, SigningKey};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct L1 {
    // The id of the settlement chain.
    pub id: String,

    // - The token that will be used to pay for tx fee in the appchain.
    // - For now, this must be the native token that is used to pay for tx fee in the settlement chain.
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
pub struct InitOutcome {
    // the initialized chain id
    pub id: String,

    // the fee token contract
    //
    // this corresponds to the l1 token contract
    pub fee_token: ContractAddress,

    pub l1: L1,
}

#[derive(Args)]
pub struct InitArgs {
    // #[arg(long = "account", value_name = "ADDRESS")]
    // pub sender_address: ContractAddress,

    // pub private_key: Felt,

    // #[arg(long = "l1.provider", value_name = "URL")]
    // pub l1_provider_url: Url,

    // /// The id of the chain to be initialized.
    // #[arg(long = "id", value_name = "ID")]
    // pub chain_id: String,

    // pub l1_fee_token: ContractAddress,

    // /// If not specified, will be deployed on-demand.
    // pub settlement_contract: Option<ContractAddress>,
    #[arg(value_name = "PATH")]
    pub output_path: Option<PathBuf>,
}

impl InitArgs {
    pub(crate) fn execute(self) -> Result<()> {
        tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(async move {
            // let account = self.account();
            // let l1_chain_id = account.provider().chain_id().await?;

            // let core_contract = init_core_contract(&account).await?;

            self.prompt()?;

            todo!();

            // let output_path =
            //     if let Some(path) = self.output_path { path } else { config_path(&self.chain_id)? };

            // TODO:
            // - deploy bridge contract
            // - generate the genesis

            // let l1 = L1 {
            //     id: parse_cairo_short_string(&l1_chain_id)?,
            //     settlement_contract: ContractAddress::default(),
            //     bridge_contract: ContractAddress::default(),
            //     fee_token: ContractAddress::default(),
            // };

            // let output =
            //     InitOutcome { l1, id: self.chain_id, fee_token: ContractAddress::default() };

            // let content = toml::to_string_pretty(&output)?;
            // std::fs::write(dbg!(output_path), content)?;

            Result::<(), anyhow::Error>::Ok(())
        })?;

        Ok(())
    }

    fn prompt(&self) -> Result<()> {
        let chain_id = Text::new("Chain id").prompt()?;

        let url = CustomType::<Url>::new("L1 RPC URL")
            .with_error_message("Please enter a valid URL")
            .prompt()?;

        let l1_provider = JsonRpcClient::new(HttpTransport::new(url));
        let l1_provider = Arc::new(l1_provider);

        let fee_token = CustomType::<ContractAddress>::new("Fee token")
            .with_parser(fee_token_parser(l1_provider.clone()))
            .with_error_message("Please enter a valid fee token")
            // .with_validator(fee_token_parser(l1_provider.clone()))
            .prompt()?;

        // If skipped, we deploy on demand.
        let settlement_contract = Text::new("Settlement contract").prompt_skippable()?;

        Ok(())
    }

    // fn account(&self) -> SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> {
    //     let provider = JsonRpcClient::new(HttpTransport::new(self.l1_provider_url.clone()));
    //     let private_key = SigningKey::from_secret_scalar(self.private_key);

    //     SingleOwnerAccount::new(
    //         provider,
    //         LocalWallet::from_signing_key(private_key),
    //         self.sender_address.into(),
    //         Felt::ONE,
    //         ExecutionEncoding::New,
    //     )
    // }
}

#[derive(Debug, thiserror::Error)]
#[error("Fee token doesn't exist")]
pub struct FeeTokenNotExist;

// pub type CustomTypeParser<'a, T> = &'a dyn Fn(&str) -> Result<T, ()>;

fn fee_token_parser(provider: impl Provider + Clone) -> impl CustomTypeParser<'a, T> {
    move |input: &str| {
        let block_id = BlockId::Tag(BlockTag::Pending);
        let result = futures::executor::block_on(provider.get_class_hash_at(block_id, input));

        match result {
            Ok(..) => Ok(Validation::Valid),
            Err(..) => Err(Box::new(FeeTokenNotExist) as CustomUserError),
        }
    }
}

async fn init_core_contract(
    account: &SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
) -> Result<ContractAddress> {
    let class =
        include_str!("../../../../../crates/katana/contracts/build/appchain_core_contract.json");
    let (contract, compiled_class_hash) = prepare_contract_declaration_params(class)?;

    let class_hash = contract.class_hash();
    let res = account.declare_v2(contract.into(), compiled_class_hash).send().await?;
    let _ = TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

    let factory = ContractFactory::new(class_hash, &account);

    // appchain::constructor() https://github.com/cartridge-gg/piltover/blob/d373a844c3428383a48518adf468bf83249dec3a/src/appchain.cairo#L119-L125
    let request = factory.deploy_v3(
        vec![
            account.address(), // owner
            Felt::ZERO,        // state_root
            Felt::ZERO,        // block_number
            Felt::ZERO,        // block_hash
        ],
        Felt::ZERO,
        false,
    );

    let res = request.send().await?;
    let _ = TransactionWaiter::new(res.transaction_hash, account.provider()).await?;

    // TODO: initialize the core contract with the right program info

    Ok(request.deployed_address().into())
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
