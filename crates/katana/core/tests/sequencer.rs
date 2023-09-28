use std::time::Duration;

use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::backend::storage::transaction::{DeclareTransaction, KnownTransaction};
use katana_core::sequencer::{KatanaSequencer, SequencerConfig};
use katana_core::utils::contract::get_contract_class;
use starknet::core::types::FieldElement;
use starknet_api::core::{ClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    DeclareTransaction as DeclareApiTransaction, DeclareTransactionV0V1, TransactionHash,
};
use starknet_api::{patricia_key, stark_felt};
use tokio::time::sleep;

fn create_test_sequencer_config() -> (SequencerConfig, StarknetConfig) {
    (
        SequencerConfig { block_time: None, ..Default::default() },
        StarknetConfig {
            seed: [0u8; 32],
            total_accounts: 2,
            disable_fee: true,
            env: Environment::default(),
            ..Default::default()
        },
    )
}

async fn create_test_sequencer() -> KatanaSequencer {
    let (sequencer_config, starknet_config) = create_test_sequencer_config();
    KatanaSequencer::new(sequencer_config, starknet_config).await
}

fn create_declare_transaction(sender_address: ContractAddress) -> DeclareTransaction {
    let compiled_class =
        get_contract_class(include_str!("../contracts/compiled/test_contract.json"));
    DeclareTransaction {
        inner: DeclareApiTransaction::V0(DeclareTransactionV0V1 {
            class_hash: ClassHash(stark_felt!("0x1234")),
            nonce: Nonce(1u8.into()),
            sender_address,
            transaction_hash: TransactionHash(stark_felt!("0x6969")),
            ..Default::default()
        }),
        compiled_class,
        sierra_class: None,
    }
}

#[tokio::test]
async fn test_next_block_timestamp_in_past() {
    let sequencer = create_test_sequencer().await;
    let block1 = sequencer.backend.mine_empty_block().await.block_number;
    let block1_timestamp = sequencer
        .backend
        .blockchain
        .storage
        .read()
        .block_by_number(block1)
        .unwrap()
        .header
        .timestamp;

    sequencer.set_next_block_timestamp(block1_timestamp - 1000).await.unwrap();

    let block2 = sequencer.backend.mine_empty_block().await.block_number;
    let block2_timestamp = sequencer
        .backend
        .blockchain
        .storage
        .read()
        .block_by_number(block2)
        .unwrap()
        .header
        .timestamp;

    assert_eq!(block2_timestamp, block1_timestamp - 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_set_next_block_timestamp_in_future() {
    let sequencer = create_test_sequencer().await;
    let block1 = sequencer.backend.mine_empty_block().await.block_number;
    let block1_timestamp = sequencer
        .backend
        .blockchain
        .storage
        .read()
        .block_by_number(block1)
        .unwrap()
        .header
        .timestamp;

    sequencer.set_next_block_timestamp(block1_timestamp + 1000).await.unwrap();

    let block2 = sequencer.backend.mine_empty_block().await.block_number;
    let block2_timestamp = sequencer
        .backend
        .blockchain
        .storage
        .read()
        .block_by_number(block2)
        .unwrap()
        .header
        .timestamp;

    assert_eq!(block2_timestamp, block1_timestamp + 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_increase_next_block_timestamp() {
    let sequencer = create_test_sequencer().await;
    let block1 = sequencer.backend.mine_empty_block().await.block_number;
    let block1_timestamp = sequencer
        .backend
        .blockchain
        .storage
        .read()
        .block_by_number(block1)
        .unwrap()
        .header
        .timestamp;

    sequencer.increase_next_block_timestamp(1000).await.unwrap();

    let block2 = sequencer.backend.mine_empty_block().await.block_number;
    let block2_timestamp = sequencer
        .backend
        .blockchain
        .storage
        .read()
        .block_by_number(block2)
        .unwrap()
        .header
        .timestamp;

    assert_eq!(block2_timestamp, block1_timestamp + 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_set_storage_at_on_instant_mode() {
    let sequencer = create_test_sequencer().await;
    sequencer.backend.mine_empty_block().await;

    let contract_address = ContractAddress(patricia_key!("0x1337"));
    let key = StorageKey(patricia_key!("0x20"));
    let val = stark_felt!("0xABC");

    {
        let mut state = sequencer.backend.state.write().await;
        let read_val = state.get_storage_at(contract_address, key).unwrap();
        assert_eq!(stark_felt!("0x0"), read_val, "latest storage value should be 0");
    }

    sequencer.set_storage_at(contract_address, key, val).await.unwrap();

    {
        let mut state = sequencer.backend.state.write().await;
        let read_val = state.get_storage_at(contract_address, key).unwrap();
        assert_eq!(val, read_val, "latest storage value incorrect after generate");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_and_load_state() {
    let sequencer_old = create_test_sequencer().await;
    assert_eq!(sequencer_old.block_number().await, 0);

    let declare_tx = create_declare_transaction(ContractAddress(patricia_key!(
        sequencer_old.backend.accounts[0].address
    )));

    let tx_hash = declare_tx.inner.transaction_hash();

    sequencer_old.add_declare_transaction(declare_tx);

    // wait for the tx to be picked up from the mempool, and executed and included in the next block
    sleep(Duration::from_millis(500)).await;

    let tx_in_storage = sequencer_old.transaction(&tx_hash.0.into()).await.unwrap();

    matches!(tx_in_storage, KnownTransaction::Included(_));
    assert_eq!(sequencer_old.block_number().await, 1);

    let serializable_state = sequencer_old
        .backend
        .state
        .read()
        .await
        .dump_state()
        .expect("must be able to serialize state");

    assert!(
        serializable_state.classes.get(&FieldElement::from_hex_be("0x1234").unwrap()).is_some(),
        "class must be serialized"
    );

    // instantiate a new sequencer with the serialized state
    let (sequencer_config, mut starknet_config) = create_test_sequencer_config();
    starknet_config.init_state = Some(serializable_state);
    let sequencer_new = KatanaSequencer::new(sequencer_config, starknet_config).await;

    let old_contract = sequencer_old
        .backend
        .state
        .write()
        .await
        .get_compiled_contract_class(&ClassHash(stark_felt!("0x1234")))
        .unwrap();

    let new_contract = sequencer_new
        .backend
        .state
        .write()
        .await
        .get_compiled_contract_class(&ClassHash(stark_felt!("0x1234")))
        .unwrap();

    assert_eq!(old_contract, new_contract);
}
