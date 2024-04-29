use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dojo_test_utils::sequencer::{get_default_test_starknet_config, TestSequencer};
use jsonrpsee::http_client::HttpClientBuilder;
use katana_core::sequencer::SequencerConfig;
use katana_rpc_api::dev::DevApiClient;
use katana_rpc_api::saya::SayaApiClient;
use katana_rpc_api::starknet::StarknetApiClient;
use katana_rpc_types::transaction::{
    TransactionsExecutionsPage, TransactionsPageCursor, CHUNK_SIZE_DEFAULT,
};
use starknet::accounts::Account;
use starknet::core::types::{FieldElement, TransactionStatus};
use tokio::time::sleep;

pub const ENOUGH_GAS: &str = "0x100000000000000000";

mod common;

#[tokio::test(flavor = "multi_thread")]
async fn no_pending_support() {
    // Saya does not support the pending block and only work on sealed blocks.
    let sequencer = TestSequencer::start(
        SequencerConfig { no_mining: true, ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    // Should return block not found on trying to fetch the pending block.
    let cursor = TransactionsPageCursor { block_number: 1, ..Default::default() };

    match client.get_transactions_executions(cursor).await {
        Ok(_) => panic!("Expected error BlockNotFound"),
        Err(e) => {
            let eo: jsonrpsee::types::ErrorObject<'_> = e.into();
            assert_eq!(eo.code(), 24);
            assert_eq!(eo.message(), "Block not found");
        }
    };
}

#[tokio::test(flavor = "multi_thread")]
async fn process_sealed_block_only() {
    // Saya does not support the pending block and only work on sealed blocks.
    let sequencer = TestSequencer::start(
        SequencerConfig { no_mining: true, ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();
    let contract = Arc::new(contract);

    // Should return successfully when no transactions have been mined on a block.
    let mut cursor = TransactionsPageCursor::default();

    let response: TransactionsExecutionsPage =
        client.get_transactions_executions(cursor).await.unwrap();

    assert!(response.transactions_executions.is_empty());
    assert_eq!(response.cursor.block_number, 1);
    assert_eq!(response.cursor.transaction_index, 0);
    assert_eq!(response.cursor.chunk_size, CHUNK_SIZE_DEFAULT);

    let declare_res = account.declare(contract.clone(), compiled_class_hash).send().await.unwrap();

    let max_retry = 10;
    let mut attempt = 0;
    loop {
        match client.transaction_status(declare_res.transaction_hash).await {
            Ok(s) => {
                if s != TransactionStatus::Received {
                    break;
                }
            }
            Err(_) => {
                assert!(attempt < max_retry);
                sleep(Duration::from_millis(300)).await;
                attempt += 1;
            }
        }
    }

    // Should still return 0 transactions execution for the block 0.
    let response: TransactionsExecutionsPage =
        client.get_transactions_executions(cursor).await.unwrap();

    assert!(response.transactions_executions.is_empty());
    assert_eq!(response.cursor.block_number, 1);
    assert_eq!(response.cursor.transaction_index, 0);
    assert_eq!(response.cursor.chunk_size, CHUNK_SIZE_DEFAULT);

    // Create block 1.
    let _: () = client.generate_block().await.unwrap();

    // Should now return 1 transaction from the mined block.
    cursor.block_number = 1;

    let response: TransactionsExecutionsPage =
        client.get_transactions_executions(cursor).await.unwrap();

    assert_eq!(response.transactions_executions.len(), 1);
    assert_eq!(response.cursor.block_number, 2);
    assert_eq!(response.cursor.transaction_index, 0);
    assert_eq!(response.cursor.chunk_size, CHUNK_SIZE_DEFAULT);
}

#[tokio::test(flavor = "multi_thread")]
async fn executions_chunks_logic_ok() {
    let sequencer = TestSequencer::start(
        SequencerConfig { no_mining: true, ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();
    let contract = Arc::new(contract);

    let declare_res = account.declare(contract.clone(), compiled_class_hash).send().await.unwrap();

    let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();
    let mut nonce = FieldElement::ONE;
    let mut last_tx_hash = FieldElement::ZERO;

    // Prepare 29 transactions to test chunks (30 at total with the previous declare).
    for i in 0..29 {
        let deploy_call =
            common::build_deploy_cairo1_contract_call(declare_res.class_hash, (i + 2_u32).into());
        let deploy_txn = account.execute(vec![deploy_call]).nonce(nonce).max_fee(max_fee);
        let tx_hash = deploy_txn.send().await.unwrap().transaction_hash;
        nonce += FieldElement::ONE;

        if i == 28 {
            last_tx_hash = tx_hash;
        }
    }

    assert!(last_tx_hash != FieldElement::ZERO);

    // Poll the statux of the last tx sent.
    let max_retry = 10;
    let mut attempt = 0;
    loop {
        match client.transaction_status(last_tx_hash).await {
            Ok(s) => {
                if s != TransactionStatus::Received {
                    break;
                }
            }
            Err(_) => {
                assert!(attempt < max_retry);
                sleep(Duration::from_millis(300)).await;
                attempt += 1;
            }
        }
    }

    // Create block 1.
    let _: () = client.generate_block().await.unwrap();

    let cursor = TransactionsPageCursor { block_number: 1, chunk_size: 15, ..Default::default() };

    let response: TransactionsExecutionsPage =
        client.get_transactions_executions(cursor).await.unwrap();
    assert_eq!(response.transactions_executions.len(), 15);
    assert_eq!(response.cursor.block_number, 1);
    assert_eq!(response.cursor.transaction_index, 15);

    // Should get the remaining 15 transactions and cursor to the next block.
    let response: TransactionsExecutionsPage =
        client.get_transactions_executions(response.cursor).await.unwrap();

    assert_eq!(response.transactions_executions.len(), 15);
    assert_eq!(response.cursor.block_number, 2);
    assert_eq!(response.cursor.transaction_index, 0);

    // Create block 2.
    let _: () = client.generate_block().await.unwrap();

    let response: TransactionsExecutionsPage =
        client.get_transactions_executions(response.cursor).await.unwrap();

    assert!(response.transactions_executions.is_empty());
    assert_eq!(response.cursor.block_number, 3);
    assert_eq!(response.cursor.transaction_index, 0);

    sequencer.stop().expect("failed to stop sequencer");
}
