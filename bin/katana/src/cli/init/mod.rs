use anyhow::{anyhow, Result};
use clap::Args;
use dojo_utils::TransactionWaiter;
use katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;
use katana_cairo::lang::starknet_classes::contract_class::ContractClass;
use katana_primitives::{felt, ContractAddress, Felt};
use starknet::accounts::{Account, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount};
use starknet::contract::ContractFactory;
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::FlattenedSierraClass;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Url};
use starknet::signers::{LocalWallet, SigningKey};

struct InitOutcome {
    // - The token that will be used to pay for tx fee in the appchain.
    // - For now, this must be the native token that is used to pay for tx fee in the settlement chain.
    fee_token: ContractAddress,

    // - The bridge contract for bridging the fee token to the appchain.
    // - This will be part of the initialization process.
    bridge_contract: ContractAddress,

    settlement_contract: ContractAddress,
}

#[derive(Args)]
pub struct InitArgs {
    // wallet
}

impl InitArgs {
    pub(crate) fn execute(self) -> Result<()> {
        let address = felt!("0x123");
        let pk = SigningKey::from_secret_scalar(felt!("0xdeadbeef"));

        let provider = JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5050")?));
        let account = SingleOwnerAccount::new(
            provider,
            LocalWallet::from_signing_key(pk),
            address,
            Felt::ONE,
            ExecutionEncoding::New,
        );

        let _contract_address = deploy_core_contracts(&account);

        // TODO: deploy bridge contract

        Ok(())
    }
}

async fn deploy_core_contracts(
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

    Ok(request.deployed_address().into())
}

pub fn prepare_contract_declaration_params(artifact: &str) -> Result<(FlattenedSierraClass, Felt)> {
    let flattened_class = get_flattened_class(artifact)
        .map_err(|e| anyhow!("error flattening the contract class: {e}"))?;
    let compiled_class_hash = get_compiled_class_hash(artifact)
        .map_err(|e| anyhow!("error computing compiled class hash: {e}"))?;
    Ok((flattened_class, compiled_class_hash))
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
