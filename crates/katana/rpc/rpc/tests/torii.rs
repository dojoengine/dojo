#![allow(deprecated)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use dojo_utils::TransactionWaiter;
use jsonrpsee::http_client::HttpClientBuilder;
use katana_node::config::sequencing::SequencingConfig;
use katana_rpc_api::dev::DevApiClient;
use katana_rpc_api::starknet::StarknetApiClient;
use katana_rpc_api::torii::ToriiApiClient;
use katana_rpc_types::transaction::{TransactionsPage, TransactionsPageCursor};
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{Call, DeclareTransactionResult, Felt, TransactionStatus};
use starknet::core::utils::get_selector_from_name;
use tokio::time::sleep;

use crate::common::prepare_contract_declaration_params;

mod common;

pub const ENOUGH_GAS: &str = "0x100000000000000000";

#[tokio::test(flavor = "multi_thread")]
async fn test_get_transactions() {
    // setup test sequencer with the given configuration
    let sequencing_config = SequencingConfig { no_mining: true, ..Default::default() };
    let sequencer = TestSequencer::start(get_default_test_config(sequencing_config)).await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) = prepare_contract_declaration_params(&path).unwrap();
    let contract = Arc::new(contract);

    // Should return successfully when no transactions have been mined.
    let cursor = TransactionsPageCursor { block_number: 0, transaction_index: 0, chunk_size: 100 };

    let response: TransactionsPage = client.get_transactions(cursor).await.unwrap();

    assert!(response.transactions.is_empty());
    assert_eq!(response.cursor.block_number, 1);
    assert_eq!(response.cursor.transaction_index, 0);

    let declare_res =
        account.declare_v2(contract.clone(), compiled_class_hash).send().await.unwrap();

    // Should return successfully with single pending txn.
    let response: TransactionsPage = client.get_transactions(response.cursor).await.unwrap();

    assert_eq!(response.transactions.len(), 1);
    assert_eq!(response.cursor.block_number, 1);
    assert_eq!(response.cursor.transaction_index, 1);

    // Create block 1.
    let _: () = client.generate_block().await.unwrap();

    // Should properly increment to new empty pending block
    let response: TransactionsPage = client.get_transactions(response.cursor).await.unwrap();

    assert!(response.transactions.is_empty());
    assert_eq!(response.cursor.block_number, 2);
    assert_eq!(response.cursor.transaction_index, 0);

    // Should block on cursor at end of page and return on new txn
    let long_poll_future = client.get_transactions(response.cursor);
    let deploy_call = build_deploy_contract_call(declare_res.class_hash, Felt::ZERO);
    let deploy_txn = account.execute_v1(vec![deploy_call]);
    let deploy_txn_future = deploy_txn.send();

    tokio::select! {
        result = long_poll_future => {
            let long_poll_result = result.unwrap();
            assert_eq!(long_poll_result.transactions.len(), 1);
            assert_eq!(long_poll_result.cursor.block_number, 2);
            assert_eq!(long_poll_result.cursor.transaction_index, 1);
        }
        result = deploy_txn_future => {
            // The declare transaction has completed, but we don't need to do anything with it here.
            result.expect("Should succeed");
        }
    }

    // Create block 2.
    let _: () = client.generate_block().await.unwrap();

    let deploy_call = build_deploy_contract_call(declare_res.class_hash, Felt::ONE);
    let deploy_txn = account.execute_v1(vec![deploy_call]);
    let deploy_txn_future = deploy_txn.send().await.unwrap();

    TransactionWaiter::new(deploy_txn_future.transaction_hash, &account.provider()).await.unwrap();

    // Should properly increment to new pending block
    let response: TransactionsPage = client
        .get_transactions(TransactionsPageCursor {
            block_number: 2,
            transaction_index: 1,
            chunk_size: 100,
        })
        .await
        .unwrap();

    assert_eq!(response.transactions.len(), 1);
    assert_eq!(response.transactions[0].0.hash, deploy_txn_future.transaction_hash);
    assert_eq!(response.cursor.block_number, 3);
    assert_eq!(response.cursor.transaction_index, 1);

    // Create block 3.
    let _: () = client.generate_block().await.unwrap();

    let max_fee = Felt::from_hex(ENOUGH_GAS).unwrap();
    let mut nonce = Felt::THREE;
    let mut last_tx_hash = Felt::ZERO;

    // Test only returns first 100 txns from pending block
    for i in 0..101 {
        let deploy_call = build_deploy_contract_call(declare_res.class_hash, (i + 2_u32).into());
        // settings the max fee manually will skip fee estimation
        let deploy_txn = account.execute_v1(vec![deploy_call]).nonce(nonce).max_fee(max_fee);
        let res = deploy_txn.send().await.unwrap();
        nonce += Felt::ONE;

        if i == 100 {
            last_tx_hash = res.transaction_hash;
        }
    }

    assert!(last_tx_hash != Felt::ZERO);

    // Poll the statux of the last tx sent.
    let max_retry = 10;
    let mut attempt = 0;
    loop {
        match client.get_transaction_status(last_tx_hash).await {
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

    let start_cursor = response.cursor;
    let response: TransactionsPage = client.get_transactions(start_cursor).await.unwrap();
    assert_eq!(response.transactions.len(), 100);
    assert_eq!(response.cursor.block_number, 4);
    assert_eq!(response.cursor.transaction_index, 100);

    // Should get one more
    let response: TransactionsPage = client.get_transactions(response.cursor).await.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(response.cursor.block_number, 4);
    assert_eq!(response.cursor.transaction_index, 101);

    // Create block 4.
    let _: () = client.generate_block().await.unwrap();

    let response: TransactionsPage = client.get_transactions(start_cursor).await.unwrap();
    assert_eq!(response.transactions.len(), 100);
    assert_eq!(response.cursor.block_number, 4);
    assert_eq!(response.cursor.transaction_index, 100);

    // Should get one more
    let response: TransactionsPage = client.get_transactions(response.cursor).await.unwrap();
    assert_eq!(response.transactions.len(), 1);
    assert_eq!(response.cursor.block_number, 5);
    assert_eq!(response.cursor.transaction_index, 0);

    sequencer.stop().expect("failed to stop sequencer");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_transactions_with_instant_mining() {
    let sequencing_config = SequencingConfig::default();
    let sequencer = TestSequencer::start(get_default_test_config(sequencing_config)).await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) = prepare_contract_declaration_params(&path).unwrap();
    let contract = Arc::new(contract);

    // Should return successfully when no transactions have been mined.
    let cursor = TransactionsPageCursor { block_number: 0, transaction_index: 0, chunk_size: 100 };

    let DeclareTransactionResult { transaction_hash, class_hash } =
        account.declare_v2(contract.clone(), compiled_class_hash).send().await.unwrap();

    dojo_utils::TransactionWaiter::new(transaction_hash, &sequencer.provider()).await.unwrap();

    // Should return successfully with single txn.
    let response: TransactionsPage = client.get_transactions(cursor).await.unwrap();

    assert_eq!(response.transactions.len(), 1);
    assert_eq!(response.cursor.block_number, 1);
    assert_eq!(response.cursor.transaction_index, 0);

    // Should block on cursor at end of page and return on new txn
    let long_poll_future = client.get_transactions(response.cursor);
    let deploy_call = build_deploy_contract_call(class_hash, Felt::ZERO);
    let deploy_txn = account.execute_v1(vec![deploy_call]);
    let deploy_txn_future = deploy_txn.send();

    tokio::select! {
        result = long_poll_future => {
            let long_poll_result = result.unwrap();
            assert_eq!(long_poll_result.transactions.len(), 1);
            assert_eq!(long_poll_result.cursor.block_number, 2);
            assert_eq!(long_poll_result.cursor.transaction_index, 0);
        }
        result = deploy_txn_future => {
            // The declare transaction has completed, but we don't need to do anything with it here.
            result.expect("Should succeed");
        }
    }

    let deploy_call = build_deploy_contract_call(class_hash, Felt::ONE);
    let deploy_txn = account.execute_v1(vec![deploy_call]);
    let deploy_txn_future = deploy_txn.send().await.unwrap();

    // Should properly increment to new pending block
    let response: TransactionsPage = client
        .get_transactions(TransactionsPageCursor {
            block_number: 2,
            transaction_index: 1,
            chunk_size: 100,
        })
        .await
        .unwrap();

    assert_eq!(response.transactions.len(), 1);
    assert_eq!(response.transactions[0].0.hash, deploy_txn_future.transaction_hash);
    assert_eq!(response.cursor.block_number, 3);
    assert_eq!(response.cursor.transaction_index, 1);

    sequencer.stop().expect("failed to stop sequencer");
}

fn build_deploy_contract_call(class_hash: Felt, salt: Felt) -> Call {
    let constructor_calldata = vec![Felt::from(1_u32), Felt::from(2_u32)];

    let calldata = [
        vec![
            class_hash,                             // class hash
            salt,                                   // salt
            Felt::ZERO,                             // unique
            Felt::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    Call {
        calldata,
        // devnet UDC address
        to: Felt::from_hex("0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf")
            .unwrap(),
        selector: get_selector_from_name("deployContract").unwrap(),
    }
}
