#![allow(deprecated)]

use std::fs::{self};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use assert_matches::assert_matches;
use cainome::rs::abigen_legacy;
use dojo_test_utils::sequencer::{get_default_test_starknet_config, TestSequencer};
use indexmap::IndexSet;
use katana_core::sequencer::SequencerConfig;
use katana_primitives::genesis::constant::{
    DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
};
use katana_rpc_types::receipt::ReceiptBlock;
use num_traits::FromPrimitive;
use starknet::accounts::{
    Account, AccountError, Call, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount,
};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionReceipt, ExecutionResult, Felt, StarknetError,
    TransactionFinalityStatus, TransactionReceipt,
};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::macros::{felt, selector};
use starknet::providers::{Provider, ProviderError};
use starknet::signers::{LocalWallet, SigningKey};

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

#[tokio::test]
async fn estimate_fee() -> Result<()> {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let provider = sequencer.provider();
    let account = sequencer.account();

    // setup contract to interact with (can be any existing contract that can be interacted with)
    abigen_legacy!(Erc20Token, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
    let contract = Erc20Token::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), &account);

    // setup contract function params
    let recipient = felt!("0x1");
    let amount = Uint256 { low: felt!("0x1"), high: Felt::ZERO };

    // send a valid transaction first to increment the nonce (so that we can test nonce < current
    // nonce later)
    let result = contract.transfer(&recipient, &amount).send().await?;

    // wait until the tx is included in a block
    dojo_utils::TransactionWaiter::new(result.transaction_hash, &provider).await?;

    // estimate fee with current nonce (the expected nonce)
    let nonce = provider.get_nonce(BlockId::Tag(BlockTag::Pending), account.address()).await?;
    let result = contract.transfer(&recipient, &amount).nonce(nonce).estimate_fee().await;
    assert!(result.is_ok(), "estimate should succeed with nonce == current nonce");

    // estimate fee with arbitrary nonce < current nonce
    //
    // here we're essentially estimating a transaction with a nonce that has already been
    // used, so it should fail.
    let nonce = nonce - 1;
    let result = contract.transfer(&recipient, &amount).nonce(nonce).estimate_fee().await;
    assert!(result.is_err(), "estimate should fail with nonce < current nonce");

    // estimate fee with arbitrary nonce >= current nonce
    let nonce = felt!("0x1337");
    let result = contract.transfer(&recipient, &amount).nonce(nonce).estimate_fee().await;
    assert!(result.is_ok(), "estimate should succeed with nonce >= current nonce");

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn rapid_transactions_submissions() -> Result<()> {
    let sequencer = TestSequencer::start(
        SequencerConfig { block_time: Some(2000), ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let account = sequencer.account();
    let provider = account.provider();

    const ITERATION: usize = 10;
    let mut txs = IndexSet::with_capacity(ITERATION);

    for _ in 0..ITERATION {
        let call = Call {
            to: DEFAULT_FEE_TOKEN_ADDRESS.into(),
            selector: selector!("transfer"),
            calldata: vec![
                felt!("0x100"), // recipient address
                Felt::ONE,      // amount (low)
                Felt::ZERO,     // amount (high)
            ],
        };

        let result = account.execute_v1(vec![call]).send().await?;
        txs.insert(result.transaction_hash);
    }

    // optimisitcally wait for 10 seconds for all the txs to be mined
    tokio::time::sleep(Duration::from_secs(5)).await;

    // we should've submitted ITERATION transactions
    assert_eq!(txs.len(), ITERATION);

    // check the status of each txs
    for hash in txs {
        let receipt = provider.get_transaction_receipt(hash).await?;
        assert_eq!(receipt.receipt.execution_result(), &ExecutionResult::Succeeded);
        assert_eq!(receipt.receipt.finality_status(), &TransactionFinalityStatus::AcceptedOnL2);
    }

    Ok(())
}

#[tokio::test]
async fn send_tx_invalid_txs() -> Result<()> {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    // setup test contract to interact with.
    abigen_legacy!(Contract, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
    let contract = Contract::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), sequencer.account());

    // function call params
    let recipient = Felt::ONE;
    let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };

    ///////////////////////////////////////////////////////////////////

    //  transaction with low max fee (underpriced).
    let res = contract.transfer(&recipient, &amount).max_fee(Felt::TWO).send().await;
    assert!(dbg!(res).is_err());

    ///////////////////////////////////////////////////////////////////

    //  transaction with insufficient balance.
    let fee = Felt::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE + 1);
    let res = contract.transfer(&recipient, &amount).max_fee(fee).send().await;
    assert!(dbg!(res).is_err());

    ///////////////////////////////////////////////////////////////////

    //  transaction with insufficient balance.
    let fee = Felt::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE + 1);
    let res = contract.transfer(&recipient, &amount).max_fee(fee).send().await;
    assert!(dbg!(res).is_err());

    ///////////////////////////////////////////////////////////////////

    //  transaction with invalid signatures.

    // starknet-rs doesn't provide a way to manually set the signatures so instead we create an
    // account with random signer that is not associated with the actual account.
    let chain_id = sequencer.provider().chain_id().await?;

    let account = SingleOwnerAccount::new(
        sequencer.provider(),
        LocalWallet::from(SigningKey::from_random()),
        sequencer.account().address(),
        chain_id,
        ExecutionEncoding::New,
    );

    let res = Contract::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), account)
        .transfer(&recipient, &amount)
        .max_fee(felt!("0x1111111111"))
        .send()
        .await;

    assert!(dbg!(res).is_err());

    ///////////////////////////////////////////////////////////////////

    Ok(())
}
