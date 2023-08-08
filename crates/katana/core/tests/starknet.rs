use blockifier::state::state_api::StateReader;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::DeclareTransaction;
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::backend::Backend;
use katana_core::db::Db;
use katana_core::utils::contract::get_contract_class;
use starknet_api::block::BlockNumber;
use starknet_api::core::{ClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    DeclareTransaction as DeclareApiTransaction, DeclareTransactionV0V1, TransactionHash,
};
use starknet_api::{patricia_key, stark_felt};

fn create_test_starknet_config() -> StarknetConfig {
    StarknetConfig {
        seed: [0u8; 32],
        auto_mine: true,
        total_accounts: 2,
        disable_fee: true,
        env: Environment::default(),
        ..Default::default()
    }
}

fn create_test_starknet() -> Backend {
    Backend::new(create_test_starknet_config())
}

fn create_declare_transaction(sender_address: ContractAddress) -> DeclareTransaction {
    let test_contract_class =
        get_contract_class(include_str!("../contracts/compiled/test_contract.json"));
    DeclareTransaction::new(
        DeclareApiTransaction::V0(DeclareTransactionV0V1 {
            class_hash: ClassHash(stark_felt!("0x1234")),
            nonce: Nonce(1u8.into()),
            sender_address,
            transaction_hash: TransactionHash(stark_felt!("0x6969")),
            ..Default::default()
        }),
        test_contract_class,
    )
    .unwrap()
}

#[tokio::test]
async fn test_next_block_timestamp_in_past() {
    let starknet = create_test_starknet();
    starknet.open_pending_block().await;

    let timestamp = starknet.env.read().block.block_timestamp;
    starknet.set_next_block_timestamp(timestamp.0 - 1000).await.unwrap();

    starknet.open_pending_block().await;
    let new_timestamp = starknet.env.read().block.block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 - 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_set_next_block_timestamp_in_future() {
    let starknet = create_test_starknet();
    starknet.open_pending_block().await;

    let timestamp = starknet.env.read().block.block_timestamp;
    starknet.set_next_block_timestamp(timestamp.0 + 1000).await.unwrap();

    starknet.open_pending_block().await;
    let new_timestamp = starknet.env.read().block.block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 + 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_increase_next_block_timestamp() {
    let starknet = create_test_starknet();
    starknet.open_pending_block().await;

    let timestamp = starknet.env.read().block.block_timestamp;
    starknet.increase_next_block_timestamp(1000).await.unwrap();

    starknet.open_pending_block().await;
    let new_timestamp = starknet.env.read().block.block_timestamp;

    assert_eq!(new_timestamp.0, timestamp.0 + 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_creating_blocks() {
    let starknet = create_test_starknet();
    starknet.open_pending_block().await;
    starknet.mine_block().await;

    assert_eq!(starknet.storage.read().await.blocks.len(), 2);
    assert_eq!(starknet.storage.read().await.latest_number, 1);
    assert_eq!(
        starknet.env.read().block.block_number,
        BlockNumber(1),
        "block context should only be updated on new pending block"
    );

    let block0 = starknet.storage.read().await.block_by_number(0).unwrap().clone();
    let block1 = starknet.storage.read().await.block_by_number(1).unwrap().clone();

    assert_eq!(block0.header.number, 0);
    assert_eq!(block1.header.number, 1);
}

#[tokio::test]
async fn dump_and_load_state() {
    let backend_old = create_test_starknet();
    backend_old.open_pending_block().await;

    let declare_tx =
        create_declare_transaction(ContractAddress(patricia_key!(backend_old.accounts[0].address)));

    backend_old
        .handle_transaction(Transaction::AccountTransaction(AccountTransaction::Declare(
            declare_tx,
        )))
        .await;

    let serializable_state =
        backend_old.state.read().await.dump_state().expect("must be able to serialize state");

    let mut starknet_config = create_test_starknet_config();
    starknet_config.init_state = Some(serializable_state);
    let backend_new = Backend::new(starknet_config);

    let old_contract = backend_old
        .state
        .write()
        .await
        .classes
        .get(&ClassHash(stark_felt!("0x1234")))
        .cloned()
        .unwrap()
        .class;

    let new_contract = backend_new
        .state
        .write()
        .await
        .classes
        .get(&ClassHash(stark_felt!("0x1234")))
        .cloned()
        .unwrap()
        .class;

    assert_eq!(old_contract, new_contract,);
}

#[tokio::test]
async fn test_set_storage_at() {
    let starknet = create_test_starknet();
    starknet.open_pending_block().await;

    let contract_address = ContractAddress(patricia_key!("0x1337"));
    let key = StorageKey(patricia_key!("0x20"));
    let val = stark_felt!("0xABC");

    starknet.set_storage_at(contract_address, key, val).await.unwrap();

    {
        let mut state = starknet.state.write().await;
        let read_val = state.get_storage_at(contract_address, key).unwrap();
        assert_eq!(stark_felt!("0x0"), read_val, "latest storage value should be 0");
    }

    {
        if let Some(pending_block) = starknet.pending_block.write().await.as_mut() {
            let read_val = pending_block.state.get_storage_at(contract_address, key).unwrap();
            assert_eq!(val, read_val, "pending set storage value incorrect");
        }
    }

    starknet.mine_block().await;

    {
        let mut state = starknet.state.write().await;
        let read_val = state.get_storage_at(contract_address, key).unwrap();
        assert_eq!(val, read_val, "latest storage value incorrect after generate");
    }
}
