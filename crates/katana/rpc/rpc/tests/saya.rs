#![allow(deprecated)]

use std::path::PathBuf;
use std::sync::Arc;

use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use dojo_utils::TransactionWaiter;
use jsonrpsee::http_client::HttpClientBuilder;
use katana_node::config::SequencingConfig;
use katana_primitives::block::{BlockIdOrTag, BlockTag};
use katana_rpc_api::dev::DevApiClient;
use katana_rpc_api::saya::SayaApiClient;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;
use starknet::macros::felt;

const ENOUGH_GAS: Felt = felt!("0x100000000000000000");

mod common;

#[tokio::test(flavor = "multi_thread")]
async fn fetch_traces_from_block() {
    let sequencer = TestSequencer::start(get_default_test_config(SequencingConfig {
        no_mining: true,
        ..Default::default()
    }))
    .await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();
    let contract = Arc::new(contract);

    let res = account.declare_v2(contract.clone(), compiled_class_hash).send().await.unwrap();
    // wait for the tx to be mined
    TransactionWaiter::new(res.transaction_hash, account.provider())
        .with_interval(200)
        .await
        .expect("tx failed");

    // Store the tx hashes to check the retrieved traces later.
    let mut tx_hashes = vec![res.transaction_hash];

    for i in 0..29 {
        let call = common::build_deploy_cairo1_contract_call(res.class_hash, (i + 2_u32).into());

        let res = account
            .execute_v1(vec![call])
            .max_fee(ENOUGH_GAS)
            .send()
            .await
            .expect("failed to send tx");

        // wait for the tx to be mined
        TransactionWaiter::new(res.transaction_hash, account.provider())
            .with_interval(200)
            .await
            .expect("tx failed");

        tx_hashes.push(res.transaction_hash);
    }

    // Generate a new block.
    let _: () = client.generate_block().await.unwrap();

    // Get the traces from the latest block.
    let traces = client
        .transaction_executions_by_block(BlockIdOrTag::Tag(BlockTag::Latest))
        .await
        .expect("failed to get traces from latest block");

    assert_eq!(
        tx_hashes.len(),
        traces.len(),
        "traces count in the latest block must equal to the total txs"
    );

    for (expected, actual) in tx_hashes.iter().zip(traces) {
        // Assert that the traces are from the txs in the requested block.
        assert_eq!(expected, &actual.hash);
    }

    sequencer.stop().expect("failed to stop sequencer");
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_traces_from_pending_block() {
    let sequencer = TestSequencer::start(get_default_test_config(SequencingConfig {
        no_mining: true,
        ..Default::default()
    }))
    .await;

    let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

    let account = sequencer.account();

    let path: PathBuf = PathBuf::from("tests/test_data/cairo1_contract.json");
    let (contract, compiled_class_hash) =
        common::prepare_contract_declaration_params(&path).unwrap();
    let contract = Arc::new(contract);

    let res = account.declare_v2(contract.clone(), compiled_class_hash).send().await.unwrap();
    // wait for the tx to be mined
    TransactionWaiter::new(res.transaction_hash, account.provider())
        .with_interval(200)
        .await
        .expect("tx failed");

    // Store the tx hashes to check the retrieved traces later.
    let mut tx_hashes = vec![res.transaction_hash];

    for i in 0..29 {
        let call = common::build_deploy_cairo1_contract_call(res.class_hash, (i + 2_u32).into());

        // we set the nonce manually so that we can send the tx rapidly without waiting for the
        // previous tx to be mined first.
        let res = account
            .execute_v1(vec![call])
            .max_fee(ENOUGH_GAS)
            .send()
            .await
            .expect("failed to send tx");

        // wait for the tx to be mined
        TransactionWaiter::new(res.transaction_hash, account.provider())
            .with_interval(200)
            .await
            .expect("tx failed");

        tx_hashes.push(res.transaction_hash);
    }

    // Get the traces from the pending block.
    let traces = client
        .transaction_executions_by_block(BlockIdOrTag::Tag(BlockTag::Pending))
        .await
        .expect("failed to get traces from pending block");

    assert_eq!(
        tx_hashes.len(),
        traces.len(),
        "traces count in the pending block must equal to the total txs"
    );

    for (expected, actual) in tx_hashes.iter().zip(traces) {
        // Assert that the traces are from the txs in the requested block.
        assert_eq!(expected, &actual.hash);
    }

    sequencer.stop().expect("failed to stop sequencer");
}
