use std::path::PathBuf;
use std::sync::Arc;

use dojo_test_utils::sequencer::{get_default_test_starknet_config, TestSequencer};
use jsonrpsee::http_client::HttpClientBuilder;
use katana_core::sequencer::SequencerConfig;
use katana_rpc_api::katana::KatanaApiClient;
use katana_rpc_api::torii::ToriiApiClient;
use katana_rpc_types::transaction::{TransactionsPage, TransactionsPageCursor};
use starknet::accounts::{Account, Call};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;

use crate::common::prepare_contract_declaration_params;

mod common;

#[tokio::test(flavor = "multi_thread")]
async fn test_get_transactions() {
    let sequencer = TestSequencer::start(
        SequencerConfig { block_time: None, no_mining: true, ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) = prepare_contract_declaration_params(&path).unwrap();
    let contract = Arc::new(contract);

    // Should return successfully when no transactions have been mined.
    let cursor = TransactionsPageCursor { block_number: 0, transaction_index: 0 };

    let response: TransactionsPage = client.get_transactions(cursor).await.unwrap();

    assert!(response.transactions.is_empty());
    assert!(response.cursor.block_number == 1);
    assert!(response.cursor.transaction_index == 0);

    let declare_res = account.declare(contract.clone(), compiled_class_hash).send().await.unwrap();

    // Should return successfully with single pending txn.
    let response: TransactionsPage = client.get_transactions(response.cursor).await.unwrap();

    assert!(response.transactions.len() == 1);
    assert!(response.cursor.block_number == 1);
    assert!(response.cursor.transaction_index == 1);

    let _: () = client.generate_block().await.unwrap();

    // Should properly increment to new empty pending block
    let response: TransactionsPage = client.get_transactions(response.cursor).await.unwrap();

    assert!(response.transactions.is_empty());
    assert!(response.cursor.block_number == 2);
    assert!(response.cursor.transaction_index == 0);

    // Should block on cursor at end of page and return on new txn
    let long_poll_future = client.get_transactions(response.cursor);

    // Yield the current task, allowing the long poll to be established.
    tokio::task::yield_now().await;

    let constructor_calldata = vec![FieldElement::from(1_u32), FieldElement::from(2_u32)];

    let calldata = [
        vec![
            declare_res.class_hash,                         // class hash
            FieldElement::ZERO,                             // salt
            FieldElement::ZERO,                             // unique
            FieldElement::from(constructor_calldata.len()), // constructor calldata len
        ],
        constructor_calldata.clone(),
    ]
    .concat();

    let deploy_txn = account.execute(vec![Call {
        calldata,
        // devnet UDC address
        to: FieldElement::from_hex_be(
            "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
        )
        .unwrap(),
        selector: get_selector_from_name("deployContract").unwrap(),
    }]);
    let deploy_txn_future = deploy_txn.send();

    tokio::select! {
        result = long_poll_future => {
            let response: TransactionsPage = result.unwrap();
            println!("{:?}", response.transactions.len());
            assert!(response.transactions.len() == 1);
            assert!(response.cursor.block_number == 2);
            assert!(response.cursor.transaction_index == 1);
        }
        _ = deploy_txn_future => {
            // The declare transaction has completed, but we don't need to do anything with it here.
        }
    }

    sequencer.stop().expect("failed to stop sequencer");
}
