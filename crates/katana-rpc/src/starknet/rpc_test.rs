use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use blockifier::state::state_api::StateReader;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use dojo_test_utils::sequencer::TestSequencer;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{
    DeclareTransactionReceipt, FieldElement, FlattenedSierraClass, MaybePendingTransactionReceipt,
    TransactionReceipt, TransactionStatus,
};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::providers::Provider;
use starknet_api::block::BlockNumber;
use starknet_api::core::{ClassHash, ContractAddress, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::{patricia_key, stark_felt};

#[tokio::test]
async fn test_send_declare_and_deploy_contract() {
    let sequencer = TestSequencer::start().await;
    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("src/starknet/test_data/cairo1_contract.json");
    let (contract, class_hash) = prepare_contract_declaration_params(&path).unwrap();

    let res = account.declare(Arc::new(contract), class_hash).send().await.unwrap();
    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { status, .. },
        )) => {
            assert_eq!(status, TransactionStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }

    let state = sequencer.sequencer.starknet.write().await.state(BlockNumber(1)).unwrap();
    assert!(state.class_hash_to_class.get(&ClassHash(stark_felt!(res.class_hash))).is_some());

    let constructor_calldata = vec![];

    let calldata = [
        vec![
            res.class_hash,                                 // class hash
            FieldElement::ZERO,                             // salt
            FieldElement::ZERO,                             // unique
            FieldElement::from(constructor_calldata.len()), // constructor calldata len
        ],
        vec![],
    ]
    .concat();

    let contract_address = get_contract_address(
        FieldElement::ZERO,
        res.class_hash,
        &constructor_calldata.clone(),
        FieldElement::ZERO,
    );

    account
        .execute(vec![Call {
            calldata,
            // devnet UDC address
            to: FieldElement::from_hex_be(
                "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
            )
            .unwrap(),
            selector: get_selector_from_name("deployContract").unwrap(),
        }])
        .send()
        .await
        .unwrap();

    let mut state = sequencer.sequencer.starknet.write().await.state(BlockNumber(1)).unwrap();
    assert!(
        state.get_class_hash_at(ContractAddress(patricia_key!(contract_address))).is_ok(),
        "contract is not deployed"
    );

    sequencer.stop().expect("failed to stop sequencer");
}

#[tokio::test]
async fn test_send_declare_and_deploy_legcay_contract() {
    let sequencer = TestSequencer::start().await;
    let account = sequencer.account();

    let path = PathBuf::from("src/starknet/test_data/cairo0_contract.json");

    let legacy_contract: LegacyContractClass =
        serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
    let contract_class = Arc::new(legacy_contract);

    let res = account.declare_legacy(contract_class).send().await.unwrap();
    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { status, .. },
        )) => {
            assert_eq!(status, TransactionStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }

    let mut state = sequencer.sequencer.starknet.write().await.state(BlockNumber(1)).unwrap();
    assert!(
        state.get_compiled_contract_class(&ClassHash(stark_felt!(res.class_hash))).is_ok(),
        "class is not declared"
    );

    let constructor_calldata = vec![FieldElement::ONE];

    let calldata = [
        vec![
            res.class_hash,                                 // class hash
            FieldElement::ZERO,                             // salt
            FieldElement::ZERO,                             // unique
            FieldElement::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    let contract_address = get_contract_address(
        FieldElement::ZERO,
        res.class_hash,
        &constructor_calldata.clone(),
        FieldElement::ZERO,
    );

    account
        .execute(vec![Call {
            calldata,
            // devnet UDC address
            to: FieldElement::from_hex_be(
                "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
            )
            .unwrap(),
            selector: get_selector_from_name("deployContract").unwrap(),
        }])
        .send()
        .await
        .unwrap();

    let mut state = sequencer.sequencer.starknet.write().await.state(BlockNumber(1)).unwrap();
    assert!(
        state.get_class_hash_at(ContractAddress(patricia_key!(contract_address))).is_ok(),
        "contract is not deployed"
    );

    sequencer.stop().expect("failed to stop sequencer");
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
