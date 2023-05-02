use std::sync::Arc;

use blockifier::abi::abi_utils::selector_from_name;
use blockifier::transaction::{
    account_transaction::AccountTransaction, transaction_execution::Transaction,
};
use katana_core::constants::{
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, FEE_ERC20_CONTRACT_ADDRESS, TEST_ACCOUNT_CONTRACT_PATH,
};
use katana_core::starknet::{transaction::ExternalFunctionCall, StarknetConfig, StarknetWrapper};
use starknet::core::{types::TransactionStatus, utils::get_selector_from_name};
use starknet_api::calldata;
use starknet_api::{
    block::BlockNumber,
    core::ContractAddress,
    core::{EntryPointSelector, PatriciaKey},
    hash::{StarkFelt, StarkHash},
    patricia_key, stark_felt,
    transaction::{Calldata, InvokeTransactionV1, TransactionHash},
};

fn create_test_starknet() -> StarknetWrapper {
    let test_account_path = [env!("CARGO_MANIFEST_DIR"), TEST_ACCOUNT_CONTRACT_PATH]
        .iter()
        .collect();

    StarknetWrapper::new(StarknetConfig {
        total_accounts: 2,
        account_path: Some(test_account_path),
    })
}

#[test]
fn test_creating_blocks() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

    assert_eq!(
        starknet.blocks.current_height,
        BlockNumber(0),
        "pending block should not be added to the chain"
    );

    starknet.generate_latest_block();
    starknet.generate_latest_block();
    starknet.generate_latest_block();

    assert_eq!(starknet.blocks.hash_to_num.len(), 3);
    assert_eq!(starknet.blocks.num_to_blocks.len(), 3);
    assert_eq!(starknet.blocks.current_height, BlockNumber(3));

    let block0 = starknet.blocks.get_by_number(BlockNumber(0)).unwrap();
    let block1 = starknet.blocks.get_by_number(BlockNumber(1)).unwrap();
    let last_block = starknet.blocks.get_lastest().unwrap();

    assert_eq!(block0.transactions(), &[]);
    assert_eq!(block0.block_number(), BlockNumber(0));
    assert_eq!(block1.block_number(), BlockNumber(1));
    assert_eq!(last_block.block_number(), BlockNumber(2));
}

#[test]
fn test_add_transaction() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

    let a = starknet.predeployed_accounts.accounts[0].clone();
    let b = starknet.predeployed_accounts.accounts[1].clone();

    println!("{}", a.account_address.0.key());
    println!("{}", b.account_address.0.key());

    let entry_point_selector = selector_from_name("transfer");
    let execute_calldata = calldata![
        stark_felt!(FEE_ERC20_CONTRACT_ADDRESS), // Contract address.
        entry_point_selector.0,                  // EP selector.
        stark_felt!(3),                          // Calldata length.
        *b.account_address.0.key(),              // Calldata: num.
        stark_felt!("0x99"),                     // Calldata: num.
        stark_felt!(0x0)                         // Calldata: num.
    ];

    starknet.handle_transaction(Transaction::AccountTransaction(AccountTransaction::Invoke(
        InvokeTransactionV1 {
            sender_address: a.account_address,
            calldata: execute_calldata,
            transaction_hash: TransactionHash(stark_felt!("0x6969")),
            ..Default::default()
        },
    )));

    let tx = starknet
        .transactions
        .transactions
        .get(&TransactionHash(stark_felt!("0x6969")));

    println!("{:?}", tx.unwrap().execution_error);

    let block = starknet.blocks.get_by_number(BlockNumber(0)).unwrap();

    assert!(tx.is_some(), "transaction must be stored");
    assert_eq!(tx.unwrap().block_number, Some(BlockNumber(0)));
    assert_eq!(starknet.blocks.current_height, BlockNumber(1));
    assert!(
        block.get_transaction_by_index(0).is_some(),
        "transaction must be included in the block"
    );
    assert_eq!(
        block
            .get_transaction_by_index(0)
            .unwrap()
            .transaction_hash(),
        TransactionHash(stark_felt!("0x6969"))
    );
    assert_eq!(tx.unwrap().status, TransactionStatus::AcceptedOnL2);
    assert_eq!(starknet.block_context.block_number, BlockNumber(1));
}

#[test]
fn test_add_reverted_transaction() {
    let mut starknet = create_test_starknet();
    starknet.generate_pending_block();

    let transaction_hash = TransactionHash(stark_felt!("0x1234"));
    let transaction =
        Transaction::AccountTransaction(AccountTransaction::Invoke(InvokeTransactionV1 {
            transaction_hash,
            ..Default::default()
        }));

    starknet.handle_transaction(transaction);

    let tx = starknet.transactions.transactions.get(&transaction_hash);

    assert_eq!(
        starknet.transactions.transactions.len(),
        1,
        "transaction must be stored even if execution fail"
    );
    assert_eq!(tx.unwrap().block_hash, None);
    assert_eq!(tx.unwrap().block_number, None);
    assert_eq!(tx.unwrap().status, TransactionStatus::Rejected);
    assert_eq!(
        starknet.blocks.current_height,
        BlockNumber(0),
        "block height must not increase"
    );
    assert_eq!(starknet.blocks.num_to_blocks.len(), 0, "no blocks added");
}

#[test]
fn test_function_call() {
    let starknet = create_test_starknet();
    let account = &starknet.predeployed_accounts.accounts[0]
        .account_address
        .0
        .key();

    let call = ExternalFunctionCall {
        calldata: Calldata(Arc::new(vec![**account])),
        contract_address: ContractAddress(patricia_key!(FEE_ERC20_CONTRACT_ADDRESS)),
        entry_point_selector: EntryPointSelector(StarkFelt::from(
            get_selector_from_name("balanceOf").unwrap(),
        )),
    };

    let res = starknet.call(call);

    assert!(res.is_ok(), "call must succeed");
    assert_eq!(
        res.unwrap().execution.retdata.0[0],
        stark_felt!(DEFAULT_PREFUNDED_ACCOUNT_BALANCE),
    );
}
