use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use dojo_test_utils::sequencer::TestSequencer;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{
    DeclareTransactionReceipt, FieldElement, FlattenedSierraClass, MaybePendingTransactionReceipt,
    TransactionReceipt, TransactionStatus,
};
use starknet::providers::Provider;

#[tokio::test]
async fn test_send_declare_v2_tx() {
    let sequencer = TestSequencer::start().await;
    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("src/starknet/test_data/cairo1_contract.json");
    let (contract, class_hash) = prepare_contract_declaration_params(&path).unwrap();

    let res = account.declare(Arc::new(contract), class_hash).send().await.unwrap();
    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    sequencer.stop().expect("failed to stop sequencer");

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { status, .. },
        )) => {
            assert_eq!(status, TransactionStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }
}

#[tokio::test]
async fn test_send_declare_v1_tx() {
    let sequencer = TestSequencer::start().await;
    let account = sequencer.account();

    let path = PathBuf::from("src/starknet/test_data/cairo0_contract.json");

    let legacy_contract: LegacyContractClass =
        serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
    let contract_class = Arc::new(legacy_contract);

    let res = account.declare_legacy(contract_class).send().await.unwrap();
    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    sequencer.stop().expect("failed to stop sequencer");

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { status, .. },
        )) => {
            assert_eq!(status, TransactionStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }
}

fn prepare_contract_declaration_params(
    artifact_path: &PathBuf,
) -> Result<(FlattenedSierraClass, FieldElement)> {
    let flattened_class = get_flattened_class(artifact_path)
        .map_err(|e| anyhow!("error flattening the contract class: {e}"))?;
    let compiled_class_hash = get_compiled_class_hash(artifact_path)
        .map_err(|e| anyhow!("error computing compiled class hash: {e}"))?;
    Ok((flattened_class, compiled_class_hash))
}

fn get_flattened_class(artifact_path: &PathBuf) -> Result<FlattenedSierraClass> {
    let file = File::open(artifact_path)?;
    let contract_artifact: SierraClass = serde_json::from_reader(&file)?;
    Ok(contract_artifact.flatten()?)
}

fn get_compiled_class_hash(artifact_path: &PathBuf) -> Result<FieldElement> {
    let file = File::open(artifact_path)?;
    let casm_contract_class: ContractClass = serde_json::from_reader(file)?;
    let casm_contract = CasmContractClass::from_contract_class(casm_contract_class, true)
        .map_err(|e| anyhow!("CasmContractClass from ContractClass error: {e}"))?;
    let res = serde_json::to_string_pretty(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}
