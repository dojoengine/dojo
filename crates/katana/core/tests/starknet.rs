use blockifier::abi::abi_utils::{get_storage_var_address, selector_from_name};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction;
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::backend::StarknetWrapper;
use katana_core::constants::FEE_TOKEN_ADDRESS;
use starknet::core::types::TransactionStatus;
use starknet_api::block::BlockNumber;
use starknet_api::core::Nonce;
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::{
    Calldata, InvokeTransaction, InvokeTransactionV1, TransactionHash,
};
use starknet_api::{calldata, stark_felt};

fn create_test_starknet() -> StarknetWrapper {
    let test_account_path =
        [env!("CARGO_MANIFEST_DIR"), "./contracts/compiled/account_without_validation.json"]
            .iter()
            .collect();

    let mut starknet = StarknetWrapper::new(StarknetConfig {
        seed: [0u8; 32],
        auto_mine: true,
        total_accounts: 2,
        allow_zero_max_fee: true,
        account_path: Some(test_account_path),
        env: Environment::default(),
    });

    starknet.generate_genesis_block();
    starknet
}

#[test]
fn test_next_block_timestamp_in_past() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

    let timestamp = starknet.block_context.block_timestamp;
    starknet.set_next_block_timestamp(timestamp.0 - 1000).unwrap();

    starknet.generate_pending_block();
    let new_timestamp = starknet.block_context.block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 - 1000, "timestamp should be updated");
}

#[test]
fn test_set_next_block_timestamp_in_future() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

    let timestamp = starknet.block_context.block_timestamp;
    starknet.set_next_block_timestamp(timestamp.0 + 1000).unwrap();

    starknet.generate_pending_block();
    let new_timestamp = starknet.block_context.block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 + 1000, "timestamp should be updated");
}

#[test]
fn test_increase_next_block_timestamp() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

    let timestamp = starknet.block_context.block_timestamp;
    starknet.increase_next_block_timestamp(1000).unwrap();

    starknet.generate_pending_block();
    let new_timestamp = starknet.block_context.block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 + 1000, "timestamp should be updated");
}

#[test]
fn test_creating_blocks() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();
    starknet.generate_latest_block();

    assert_eq!(starknet.blocks.hash_to_num.len(), 2);
    assert_eq!(starknet.blocks.num_to_block.len(), 2);
    assert_eq!(
        starknet.block_context.block_number,
        BlockNumber(1),
        "block context should only be updated on new pending block"
    );

    let block0 = starknet.blocks.by_number(BlockNumber(0)).unwrap();
    let block1 = starknet.blocks.by_number(BlockNumber(1)).unwrap();
    let last_block = starknet.blocks.latest().unwrap();

    assert_eq!(block0.transactions().len(), 4, "genesis block should have 4 transactions");
    assert_eq!(block0.block_number(), BlockNumber(0));
    assert_eq!(block1.block_number(), BlockNumber(1));
    assert_eq!(last_block.block_number(), BlockNumber(1));
}

#[test]
fn test_add_transaction() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

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

    starknet.handle_transaction(Transaction::AccountTransaction(AccountTransaction::Invoke(
        InvokeTransaction::V1(InvokeTransactionV1 {
            sender_address: a.account_address,
            calldata: execute_calldata,
            transaction_hash: TransactionHash(stark_felt!("0x6969")),
            nonce: Nonce(1u8.into()),
            ..Default::default()
        }),
    )));

    // SEND INVOKE TRANSACTION
    //

    let tx = starknet.transactions.transactions.get(&TransactionHash(stark_felt!("0x6969")));

    let block = starknet.blocks.by_number(BlockNumber(1)).unwrap();

    assert!(tx.is_some(), "transaction must be stored");
    assert_eq!(tx.unwrap().block_number, Some(BlockNumber(1)));
    assert!(block.transaction_by_index(0).is_some(), "transaction must be included in the block");
    assert_eq!(
        block.transaction_by_index(0).unwrap().transaction_hash(),
        TransactionHash(stark_felt!("0x6969"))
    );
    assert_eq!(tx.unwrap().status, TransactionStatus::AcceptedOnL2);

    // CHECK THAT THE BALANCE IS UPDATED
    //

    println!("FEE Address : {}", *FEE_TOKEN_ADDRESS);
    println!(
        "STORAGE ADDR : {}",
        get_storage_var_address("ERC20_balances", &[*a.account_address.0.key()]).unwrap().0.key()
    );

    // println!(
    //     "After {:?}",
    //     starknet.state.state.storage_view.get(&(
    //         ContractAddress(patricia_key!(FEE_ERC20_CONTRACT_ADDRESS)),
    //         get_storage_var_address("ERC20_balances", &[*a.account_address.0.key()]).unwrap()
    //     ))
    // );
}

#[test]
fn test_add_reverted_transaction() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

    let transaction_hash = TransactionHash(stark_felt!("0x1234"));
    let transaction = Transaction::AccountTransaction(AccountTransaction::Invoke(
        InvokeTransaction::V1(InvokeTransactionV1 { transaction_hash, ..Default::default() }),
    ));

    starknet.handle_transaction(transaction);

    let tx = starknet.transactions.transactions.get(&transaction_hash);

    assert_eq!(
        starknet.transactions.transactions.len(),
        5,
        "transaction must be stored even if execution fail"
    );
    assert_eq!(tx.unwrap().block_hash, None);
    assert_eq!(tx.unwrap().block_number, None);
    assert_eq!(tx.unwrap().status, TransactionStatus::Rejected);
    assert_eq!(
        starknet.blocks.num_to_block.len(),
        1,
        "no new block should be created if tx failed"
    );
}

// #[test]
// fn test_function_call() {
//     let starknet = create_test_starknet();
//     let account = &starknet.predeployed_accounts.accounts[0]
//         .account_address
//         .0
//         .key();

//     let call = ExternalFunctionCall {
//         calldata: Calldata(Arc::new(vec![**account])),
//         contract_address: ContractAddress(patricia_key!(FEE_ERC20_CONTRACT_ADDRESS)),
//         entry_point_selector: EntryPointSelector(StarkFelt::from(
//             get_selector_from_name("balanceOf").unwrap(),
//         )),
//     };

//     let res = starknet.call(call);

//     assert!(res.is_ok(), "call must succeed");
//     assert_eq!(
//         res.unwrap().execution.retdata.0[0],
//         stark_felt!(DEFAULT_PREFUNDED_ACCOUNT_BALANCE),
//     );
// }
