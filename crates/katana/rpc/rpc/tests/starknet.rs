use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use dojo_world::utils::TransactionWaiter;
use katana_primitives::FieldElement;
use katana_runner::{AnvilRunner, KatanaRunner};
use serde_json::json;
use starknet::accounts::{Account, Call, ConnectedAccount};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionReceipt, MaybePendingTransactionReceipt,
    TransactionFinalityStatus, TransactionReceipt,
};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::macros::felt;
use starknet::providers::Provider;
use tempfile::tempdir;

mod common;
use alloy::primitives::{Address, Uint, U256};
use alloy::sol;

const WAIT_TX_DELAY_MILLIS: u64 = 1000;

#[tokio::test(flavor = "multi_thread")]
async fn test_send_declare_and_deploy_contract() {
    let katana_runner = KatanaRunner::new().unwrap();
    let account = katana_runner.account(0);

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();

    let class_hash = contract.class_hash();
    let res = account.declare(Arc::new(contract), compiled_class_hash).send().await.unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { finality_status, .. },
        )) => {
            assert_eq!(finality_status, TransactionFinalityStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }

    assert!(account.provider().get_class(BlockId::Tag(BlockTag::Latest), class_hash).await.is_ok());

    let constructor_calldata = vec![FieldElement::from(1_u32), FieldElement::from(2_u32)];

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
        &constructor_calldata,
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

    drop(katana_runner);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_send_declare_and_deploy_legacy_contract() {
    let katana_runner = KatanaRunner::new().unwrap();
    let account = katana_runner.account(0);

    let path = PathBuf::from("tests/test_data/cairo0_contract.json");

    let legacy_contract: LegacyContractClass =
        serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
    let contract_class = Arc::new(legacy_contract);

    let class_hash = contract_class.class_hash().unwrap();
    let res = account.declare_legacy(contract_class).send().await.unwrap();
    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    let receipt = account.provider().get_transaction_receipt(res.transaction_hash).await.unwrap();

    match receipt {
        MaybePendingTransactionReceipt::Receipt(TransactionReceipt::Declare(
            DeclareTransactionReceipt { finality_status, .. },
        )) => {
            assert_eq!(finality_status, TransactionFinalityStatus::AcceptedOnL2);
        }
        _ => panic!("invalid tx receipt"),
    }

    assert!(account.provider().get_class(BlockId::Tag(BlockTag::Latest), class_hash).await.is_ok());

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

    drop(katana_runner)
}

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    StarknetContract,
    "tests/test_data/solidity/StarknetMessagingLocalCompiled.json"
);

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    Contract1,
    "tests/test_data/solidity/Contract1Compiled.json"
);

#[tokio::test(flavor = "multi_thread")]
async fn test_messaging_l1_l2() {
    // Prepare Anvil + Messaging Contracts
    let anvil_runner = AnvilRunner::new().await.unwrap();

    let contract_strk = StarknetContract::deploy(anvil_runner.provider()).await.unwrap();
    let strk_address = contract_strk.address();

    assert_eq!(
        contract_strk.address(),
        &Address::from_str("0x5fbdb2315678afecb367f032d93f642f64180aa3").unwrap()
    );

    let contract_c1 = Contract1::deploy(anvil_runner.provider(), *strk_address).await.unwrap();

    assert_eq!(
        contract_c1.address(),
        &Address::from_str("0xe7f1725e7734ce288f8367e1bb143e90bb3f0512").unwrap()
    );

    // Prepare Katana + Messaging Contract
    let messagin_config = json!({
        "chain": "ethereum",
        "rpc_url": anvil_runner.endpoint,
        "contract_address": contract_strk.address().to_string(),
        "sender_address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "private_key": "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        "interval": 2,
        "from_block": 0
    });
    let serialized_json = &messagin_config.to_string();

    let dir = tempdir().expect("Error creating temp dir");
    let file_path = dir.path().join("temp-anvil-messaging.json");

    // Write JSON string to a tempfile
    let mut file = File::create(&file_path).expect("Error creating temp file");
    file.write_all(serialized_json.as_bytes()).expect("Failed to write to file");

    let katana_runner =
        KatanaRunner::new_with_messaging(file_path.to_str().unwrap().to_string()).unwrap();
    let starknet_account = katana_runner.account(0);

    let path: PathBuf = PathBuf::from("tests/test_data/cairo_l1_msg_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();

    let class_hash = contract.class_hash();
    let res =
        starknet_account.declare(Arc::new(contract), compiled_class_hash).send().await.unwrap();

    let receipt = TransactionWaiter::new(res.transaction_hash, starknet_account.provider())
        .with_tx_status(TransactionFinalityStatus::AcceptedOnL2)
        .await
        .expect("Invalid tx receipt");

    // Following 2 asserts are to make sure contract declaration went through and was processed
    // successfully
    assert_eq!(receipt.finality_status(), &TransactionFinalityStatus::AcceptedOnL2);

    assert!(
        starknet_account
            .provider()
            .get_class(BlockId::Tag(BlockTag::Latest), class_hash)
            .await
            .is_ok()
    );

    let constructor_calldata = vec![];

    let calldata = [
        vec![
            class_hash,                                     // class hash
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
        &constructor_calldata,
        FieldElement::ZERO,
    );

    assert_eq!(
        contract_address,
        felt!("0x033d18fcfd3ae75ae4e8a275ce649220ed718b68dc53425b388fedcdbeab5097")
    );

    let transaction = starknet_account
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

    // wait for the tx to be mined
    TransactionWaiter::new(transaction.transaction_hash, starknet_account.provider())
        .with_tx_status(TransactionFinalityStatus::AcceptedOnL2)
        .await
        .expect("Invalid tx receipt");

    let builder = contract_c1
        .sendMessage(
            U256::from_str("0x033d18fcfd3ae75ae4e8a275ce649220ed718b68dc53425b388fedcdbeab5097")
                .unwrap(),
            U256::from_str("0x005421de947699472df434466845d68528f221a52fce7ad2934c5dae2e1f1cdc")
                .unwrap(),
            vec![U256::from(123)],
        )
        .gas(12000000)
        .value(Uint::from(1));

    // Messaging between L1 -> L2
    let receipt = builder
        .send()
        .await
        .expect("Error Await pending transaction")
        .get_receipt()
        .await
        .expect("Error getting transaction receipt");

    assert!(receipt.status());

    // wait for the tx to be mined (Using delay cause the transaction is sent from L1 and is
    // received in L2)
    tokio::time::sleep(Duration::from_millis(WAIT_TX_DELAY_MILLIS)).await;

    assert_eq!(
        starknet_account
            .provider()
            .get_block_transaction_count(BlockId::Number(3u64))
            .await
            .unwrap(),
        1
    );
}
