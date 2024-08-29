#![allow(deprecated)]

use std::fs::{self};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use assert_matches::assert_matches;
use cainome::rs::abigen_legacy;
use common::split_felt;
use dojo_test_utils::sequencer::{get_default_test_starknet_config, TestSequencer};
use indexmap::IndexSet;
use katana_core::sequencer::SequencerConfig;
use katana_primitives::genesis::constant::{
    DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_PREFUNDED_ACCOUNT_BALANCE,
};
use katana_rpc_types::receipt::ReceiptBlock;
use starknet::accounts::{
    Account, AccountError, Call, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount,
};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionReceipt, ExecutionResult, Felt, StarknetError,
    TransactionFinalityStatus, TransactionReceipt,
};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::macros::felt;
use starknet::providers::{Provider, ProviderError};
use starknet::signers::{LocalWallet, SigningKey};
use tokio::sync::Mutex;

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
    let res = contract.transfer(&recipient, &amount).send().await?;
    dojo_utils::TransactionWaiter::new(res.transaction_hash, &provider).await?;

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

#[rstest::rstest]
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_transactions_submissions(
    #[values(None, Some(1000))] block_time: Option<u64>,
) -> Result<()> {
    // setup test sequencer with the given configuration
    let starknet_config = get_default_test_starknet_config();
    let sequencer_config = SequencerConfig { block_time, ..Default::default() };

    let sequencer = TestSequencer::start(sequencer_config, starknet_config).await;
    let provider = sequencer.provider();
    let account = Arc::new(sequencer.account());

    // setup test contract to interact with.
    abigen_legacy!(Contract, "crates/katana/rpc/rpc/tests/test_data/erc20.json");

    // function call params
    let recipient = Felt::ONE;
    let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };

    let initial_nonce =
        provider.get_nonce(BlockId::Tag(BlockTag::Pending), sequencer.account().address()).await?;

    const N: usize = 100;
    let nonce = Arc::new(Mutex::new(initial_nonce));
    let txs = Arc::new(Mutex::new(IndexSet::with_capacity(N)));

    let mut handles = Vec::with_capacity(N);

    for _ in 0..N {
        let txs = txs.clone();
        let nonce = nonce.clone();
        let amount = amount.clone();
        let account = account.clone();

        let handle = tokio::spawn(async move {
            let mut nonce = nonce.lock().await;
            let contract = Contract::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), account);
            let res = contract.transfer(&recipient, &amount).nonce(*nonce).send().await.unwrap();
            txs.lock().await.insert(res.transaction_hash);
            *nonce += Felt::ONE;
        });

        handles.push(handle);
    }

    // wait for all txs to be submitted
    for handle in handles {
        handle.await?;
    }

    // Wait only for the last transaction to be accepted
    let txs = txs.lock().await;
    let last_tx = txs.last().unwrap();
    dojo_utils::TransactionWaiter::new(*last_tx, &provider).await?;

    // we should've submitted ITERATION transactions
    assert_eq!(txs.len(), N);

    // check the status of each txs
    for hash in txs.iter() {
        let receipt = provider.get_transaction_receipt(hash).await?;
        assert_eq!(receipt.receipt.execution_result(), &ExecutionResult::Succeeded);
        assert_eq!(receipt.receipt.finality_status(), &TransactionFinalityStatus::AcceptedOnL2);
    }

    let nonce = account.get_nonce().await?;
    assert_eq!(nonce, Felt::from(N), "Nonce should be incremented by {N} time");

    Ok(())
}

/// Macro used to assert that the given error is a Starknet error.
macro_rules! assert_starknet_err {
    ($err:expr, $api_err:pat) => {
        assert_matches!($err, AccountError::Provider(ProviderError::StarknetError($api_err)))
    };
}

#[rstest::rstest]
#[tokio::test]
async fn ensure_validator_have_valid_state(
    #[values(None, Some(1000))] block_time: Option<u64>,
) -> Result<()> {
    let mut starknet_config = get_default_test_starknet_config();
    starknet_config.disable_fee = false;
    let sequencer_config = SequencerConfig { block_time, ..Default::default() };

    let sequencer = TestSequencer::start(sequencer_config, starknet_config).await;
    let account = sequencer.account();

    // setup test contract to interact with.
    abigen_legacy!(Contract, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
    let contract = Contract::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), &account);

    // reduce account balance
    let recipient = felt!("0x1337");
    let (low, high) = split_felt(Felt::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE / 2));
    let amount = Uint256 { low, high };

    let res = contract.transfer(&recipient, &amount).send().await?;
    dojo_utils::TransactionWaiter::new(res.transaction_hash, &sequencer.provider()).await?;

    // this should fail validation due to insufficient balance because we specify max fee > the
    // actual balance that we have now.
    let fee = Felt::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE);
    let err = contract.transfer(&recipient, &amount).max_fee(fee).send().await.unwrap_err();
    assert_starknet_err!(err, StarknetError::InsufficientAccountBalance);

    Ok(())
}

#[rstest::rstest]
#[tokio::test]
async fn send_txs_with_insufficient_fee(
    #[values(true, false)] disable_fee: bool,
    #[values(None, Some(1000))] block_time: Option<u64>,
) -> Result<()> {
    // setup test sequencer with the given configuration
    let mut starknet_config = get_default_test_starknet_config();
    starknet_config.disable_fee = disable_fee;
    let sequencer_config = SequencerConfig { block_time, ..Default::default() };

    let sequencer = TestSequencer::start(sequencer_config, starknet_config).await;

    // setup test contract to interact with.
    abigen_legacy!(Contract, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
    let contract = Contract::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), sequencer.account());

    // function call params
    let recipient = Felt::ONE;
    let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };

    // initial sender's account nonce. use to assert how the txs validity change the account nonce.
    let initial_nonce = sequencer.account().get_nonce().await?;

    // -----------------------------------------------------------------------
    //  transaction with low max fee (underpriced).

    let res = contract.transfer(&recipient, &amount).max_fee(Felt::TWO).send().await;

    if disable_fee {
        // in no fee mode, setting the max fee (which translates to the tx run resources) lower
        // than the amount required would result in a validation failure. due to insufficient
        // resources.
        assert_starknet_err!(res.unwrap_err(), StarknetError::ValidationFailure(_));
    } else {
        assert_starknet_err!(res.unwrap_err(), StarknetError::InsufficientMaxFee);
    }

    let nonce = sequencer.account().get_nonce().await?;
    assert_eq!(initial_nonce, nonce, "Nonce shouldn't change after invalid tx");

    // -----------------------------------------------------------------------
    //  transaction with insufficient balance.

    let fee = Felt::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE + 1);
    let res = contract.transfer(&recipient, &amount).max_fee(fee).send().await;

    if disable_fee {
        // in no fee mode, account balance is ignored. as long as the max fee (aka resources) is
        // enough to at least run the account validation, the tx should be accepted.
        // Wait for the transaction to be accepted
        dojo_utils::TransactionWaiter::new(res?.transaction_hash, &sequencer.provider()).await?;

        // nonce should be incremented by 1 after a valid tx.
        let nonce = sequencer.account().get_nonce().await?;
        assert_eq!(initial_nonce + 1, nonce);
    } else {
        assert_starknet_err!(res.unwrap_err(), StarknetError::InsufficientAccountBalance);

        // nonce shouldn't change for an invalid tx.
        let nonce = sequencer.account().get_nonce().await?;
        assert_eq!(initial_nonce, nonce);
    }

    Ok(())
}

#[rstest::rstest]
#[tokio::test]
async fn send_txs_with_invalid_signature(
    #[values(true, false)] disable_validate: bool,
    #[values(None, Some(1000))] block_time: Option<u64>,
) -> Result<()> {
    // setup test sequencer with the given configuration
    let mut starknet_config = get_default_test_starknet_config();
    starknet_config.disable_validate = disable_validate;
    let sequencer_config = SequencerConfig { block_time, ..Default::default() };

    let sequencer = TestSequencer::start(sequencer_config, starknet_config).await;

    // starknet-rs doesn't provide a way to manually set the signatures so instead we create an
    // account with random signer to simulate invalid signatures.

    let account = SingleOwnerAccount::new(
        sequencer.provider(),
        LocalWallet::from(SigningKey::from_random()),
        sequencer.account().address(),
        sequencer.provider().chain_id().await?,
        ExecutionEncoding::New,
    );

    // setup test contract to interact with.
    abigen_legacy!(Contract, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
    let contract = Contract::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), &account);

    // function call params
    let recipient = Felt::ONE;
    let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };

    // initial sender's account nonce. use to assert how the txs validity change the account nonce.
    let initial_nonce = account.get_nonce().await?;

    // -----------------------------------------------------------------------
    //  transaction with invalid signatures.

    // we set the max fee manually here to skip fee estimation. we want to test the pool validator.
    let res = contract.transfer(&recipient, &amount).max_fee(felt!("0x1111111111")).send().await;

    if disable_validate {
        // Wait for the transaction to be accepted
        dojo_utils::TransactionWaiter::new(res?.transaction_hash, &sequencer.provider()).await?;

        // nonce should be incremented by 1 after a valid tx.
        let nonce = sequencer.account().get_nonce().await?;
        assert_eq!(initial_nonce + 1, nonce);
    } else {
        assert_starknet_err!(res.unwrap_err(), StarknetError::ValidationFailure(_));

        // nonce shouldn't change for an invalid tx.
        let nonce = sequencer.account().get_nonce().await?;
        assert_eq!(initial_nonce, nonce);
    }

    Ok(())
}

#[rstest::rstest]
#[tokio::test]
async fn send_txs_with_invalid_nonces(
    #[values(None, Some(1000))] block_time: Option<u64>,
) -> Result<()> {
    // setup test sequencer with the given configuration
    let starknet_config = get_default_test_starknet_config();
    let sequencer_config = SequencerConfig { block_time, ..Default::default() };

    let sequencer = TestSequencer::start(sequencer_config, starknet_config).await;
    let provider = sequencer.provider();
    let account = sequencer.account();

    // setup test contract to interact with.
    abigen_legacy!(Contract, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
    let contract = Contract::new(DEFAULT_FEE_TOKEN_ADDRESS.into(), &account);

    // function call params
    let recipient = Felt::ONE;
    let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };

    // set the fee manually here to skip fee estimation. we want to test the pool validator.
    let fee = felt!("0x11111111111");

    // send a valid transaction first to increment the nonce (so that we can test nonce < current
    // nonce later)
    let res = contract.transfer(&recipient, &amount).send().await?;
    dojo_utils::TransactionWaiter::new(res.transaction_hash, &provider).await?;

    // initial sender's account nonce. use to assert how the txs validity change the account nonce.
    let initial_nonce = account.get_nonce().await?;
    assert_eq!(initial_nonce, Felt::ONE, "Initial nonce after sending 1st tx should be 1.");

    // -----------------------------------------------------------------------
    //  transaction with nonce < account nonce.

    let old_nonce = initial_nonce - Felt::ONE;
    let res = contract.transfer(&recipient, &amount).nonce(old_nonce).max_fee(fee).send().await;
    assert_starknet_err!(res.unwrap_err(), StarknetError::InvalidTransactionNonce);

    let nonce = account.get_nonce().await?;
    assert_eq!(nonce, initial_nonce, "Nonce shouldn't change on invalid tx.");

    // -----------------------------------------------------------------------
    //  transaction with nonce = account nonce.

    let curr_nonce = initial_nonce;
    let res = contract.transfer(&recipient, &amount).nonce(curr_nonce).max_fee(fee).send().await?;
    dojo_utils::TransactionWaiter::new(res.transaction_hash, &provider).await?;

    let nonce = account.get_nonce().await?;
    assert_eq!(nonce, Felt::TWO, "Nonce should be 2 after sending two valid txs.");

    // -----------------------------------------------------------------------
    //  transaction with nonce >= account nonce.
    //
    // ideally, tx with nonce >= account nonce should be considered as valid BUT not to be executed
    // immediately and should be kept around in the pool until the nonce is reached. however,
    // katana doesn't support this feature yet so the current behaviour is to treat the tx as
    // invalid with nonce mismatch error.

    let new_nonce = felt!("0x100");
    let res = contract.transfer(&recipient, &amount).nonce(new_nonce).max_fee(fee).send().await;
    assert_starknet_err!(res.unwrap_err(), StarknetError::InvalidTransactionNonce);

    let nonce = account.get_nonce().await?;
    assert_eq!(nonce, Felt::TWO, "Nonce shouldn't change bcs the tx is still invalid.");

    Ok(())
}
