use blockifier::abi::abi_utils::selector_from_name;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction;
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::backend::Backend;
use katana_core::constants::FEE_TOKEN_ADDRESS;
use starknet::core::types::FieldElement;
use starknet_api::block::BlockNumber;
use starknet_api::core::Nonce;
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::{
    Calldata, InvokeTransaction, InvokeTransactionV1, TransactionHash,
};
use starknet_api::{calldata, stark_felt};

async fn create_test_starknet() -> Backend {
    let test_account_path =
        [env!("CARGO_MANIFEST_DIR"), "./contracts/compiled/account_without_validation.json"]
            .iter()
            .collect();

    Backend::new(StarknetConfig {
        seed: [0u8; 32],
        auto_mine: true,
        total_accounts: 2,
        allow_zero_max_fee: true,
        account_path: Some(test_account_path),
        env: Environment::default(),
    })
}

#[tokio::test]
async fn test_next_block_timestamp_in_past() {
    let starknet = create_test_starknet().await;
    starknet.generate_pending_block().await;

    let timestamp = starknet.block_context.read().block_timestamp;
    starknet.set_next_block_timestamp(timestamp.0 - 1000).await.unwrap();

    starknet.generate_pending_block().await;
    let new_timestamp = starknet.block_context.read().block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 - 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_set_next_block_timestamp_in_future() {
    let starknet = create_test_starknet().await;
    starknet.generate_pending_block().await;

    let timestamp = starknet.block_context.read().block_timestamp;
    starknet.set_next_block_timestamp(timestamp.0 + 1000).await.unwrap();

    starknet.generate_pending_block().await;
    let new_timestamp = starknet.block_context.read().block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 + 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_increase_next_block_timestamp() {
    let starknet = create_test_starknet().await;
    starknet.generate_pending_block().await;

    let timestamp = starknet.block_context.read().block_timestamp;
    starknet.increase_next_block_timestamp(1000).await.unwrap();

    starknet.generate_pending_block().await;
    let new_timestamp = starknet.block_context.read().block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 + 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_creating_blocks() {
    let starknet = create_test_starknet().await;
    starknet.generate_pending_block().await;
    starknet.generate_latest_block().await;

    assert_eq!(starknet.storage.read().await.blocks.len(), 2);
    assert_eq!(starknet.storage.read().await.latest_number, 1);
    assert_eq!(
        starknet.block_context.read().block_number,
        BlockNumber(1),
        "block context should only be updated on new pending block"
    );

    let block0 = starknet.storage.read().await.block_by_number(0).unwrap().clone();
    let block1 = starknet.storage.read().await.block_by_number(1).unwrap().clone();

    assert_eq!(block0.header.number, 0);
    assert_eq!(block1.header.number, 1);
}

#[tokio::test]
async fn test_add_transaction() {
    let starknet = create_test_starknet().await;
    starknet.generate_pending_block().await;

    let a = starknet.predeployed_accounts.accounts[0].clone();
    let b = starknet.predeployed_accounts.accounts[1].clone();

    // CREATE `transfer` INVOKE TRANSACTION
    //

    let entry_point_selector = selector_from_name("transfer");
    let execute_calldata = calldata![
        *FEE_TOKEN_ADDRESS,         // Contract address.
        entry_point_selector.0,     // EP selector.
        stark_felt!(3_u8),          // Calldata length.
        *b.account_address.0.key(), // Calldata: num.
        stark_felt!("0x99"),        // Calldata: num.
        stark_felt!(0_u8)           // Calldata: num.
    ];

    starknet
        .handle_transaction(Transaction::AccountTransaction(AccountTransaction::Invoke(
            InvokeTransaction::V1(InvokeTransactionV1 {
                sender_address: a.account_address,
                calldata: execute_calldata,
                transaction_hash: TransactionHash(stark_felt!("0x6969")),
                nonce: Nonce(1u8.into()),
                ..Default::default()
            }),
        )))
        .await;

    // SEND INVOKE TRANSACTION
    //

    let tx = starknet
        .storage
        .read()
        .await
        .transactions
        .get(&FieldElement::from(0x6969u64))
        .cloned()
        .unwrap();

    let block = starknet.storage.read().await.block_by_number(1).cloned().unwrap();

    assert!(tx.is_included());
    assert_eq!(
        block.transactions[0].transaction.transaction_hash(),
        TransactionHash(stark_felt!("0x6969"))
    );
}

#[tokio::test]
async fn test_add_reverted_transaction() {
    let starknet = create_test_starknet().await;
    starknet.generate_pending_block().await;

    let transaction_hash = TransactionHash(stark_felt!("0x1234"));
    let transaction = Transaction::AccountTransaction(AccountTransaction::Invoke(
        InvokeTransaction::V1(InvokeTransactionV1 { transaction_hash, ..Default::default() }),
    ));

    starknet.handle_transaction(transaction).await;

    assert_eq!(
        starknet.storage.read().await.transactions.len(),
        1,
        "transaction must be stored even if execution fail"
    );
    assert_eq!(
        starknet.storage.read().await.total_blocks(),
        1,
        "no new block should be created if tx failed"
    );
}
