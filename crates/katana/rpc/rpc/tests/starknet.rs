#![allow(deprecated)]

use std::fs::{self};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dojo_test_utils::sequencer::{get_default_test_starknet_config, TestSequencer};
use katana_core::sequencer::SequencerConfig;
use katana_rpc_types::receipt::ReceiptBlock;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionReceipt, Felt, TransactionFinalityStatus,
    TransactionReceipt,
};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::providers::Provider;

mod common;

const WAIT_TX_DELAY_MILLIS: u64 = 1000;

#[tokio::test(flavor = "multi_thread")]
async fn test_send_declare_and_deploy_contract() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();

    let class_hash = contract.class_hash();
    let res = account.declare_v2(Arc::new(contract), compiled_class_hash).send().await.unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt.block {
        ReceiptBlock::Block { .. } => {
            let TransactionReceipt::Declare(DeclareTransactionReceipt { finality_status, .. }) =
                receipt.receipt
            else {
                panic!("invalid tx receipt")
            };

            assert_eq!(finality_status, TransactionFinalityStatus::AcceptedOnL2);
        }

        _ => panic!("invalid tx receipt"),
    }

    assert!(account.provider().get_class(BlockId::Tag(BlockTag::Latest), class_hash).await.is_ok());

    let constructor_calldata = vec![Felt::from(1_u32), Felt::from(2_u32)];

    let calldata = [
        vec![
            res.class_hash,                         // class hash
            Felt::ZERO,                             // salt
            Felt::ZERO,                             // unique
            Felt::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    let contract_address =
        get_contract_address(Felt::ZERO, res.class_hash, &constructor_calldata, Felt::ZERO);

    account
        .execute_v1(vec![Call {
            calldata,
            // devnet UDC address
            to: Felt::from_hex("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf")
                .unwrap(),
            selector: get_selector_from_name("deployContract").unwrap(),
        }])
        .send()
        .await
        .unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    assert_eq!(
        account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Latest), contract_address)
            .await
            .unwrap(),
        class_hash
    );

    sequencer.stop().expect("failed to stop sequencer");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_declare_and_deploy_legacy_contract() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();

    let path = PathBuf::from("tests/test_data/cairo0_contract.json");

    let legacy_contract: LegacyContractClass =
        serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
    let contract_class = Arc::new(legacy_contract);

    let class_hash = contract_class.class_hash().unwrap();
    let res = account.declare_legacy(contract_class).send().await.unwrap();
    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt.block {
        ReceiptBlock::Block { .. } => {
            let TransactionReceipt::Declare(DeclareTransactionReceipt { finality_status, .. }) =
                receipt.receipt
            else {
                panic!("invalid tx receipt")
            };

            assert_eq!(finality_status, TransactionFinalityStatus::AcceptedOnL2);
        }

        _ => panic!("invalid tx receipt"),
    }

    assert!(account.provider().get_class(BlockId::Tag(BlockTag::Latest), class_hash).await.is_ok());

    let constructor_calldata = vec![Felt::ONE];

    let calldata = [
        vec![
            res.class_hash,                         // class hash
            Felt::ZERO,                             // salt
            Felt::ZERO,                             // unique
            Felt::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    let contract_address =
        get_contract_address(Felt::ZERO, res.class_hash, &constructor_calldata.clone(), Felt::ZERO);

    account
        .execute_v1(vec![Call {
            calldata,
            // devnet UDC address
            to: Felt::from_hex("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf")
                .unwrap(),
            selector: get_selector_from_name("deployContract").unwrap(),
        }])
        .send()
        .await
        .unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    assert_eq!(
        account
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Latest), contract_address)
            .await
            .unwrap(),
        class_hash
    );

    sequencer.stop().expect("failed to stop sequencer");
}
