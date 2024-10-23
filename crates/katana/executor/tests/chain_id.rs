mod fixtures;

// 0x151341532499876
use std::sync::Arc;

use cainome::rs::abigen;
use dojo_test_utils::sequencer::{get_default_test_config, SequencingConfig, TestSequencer};
use katana_primitives::chain::ChainId;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{GenesisAllocation, GenesisContractAlloc};
use katana_primitives::genesis::constant::read_compiled_class_artifact;
use katana_primitives::genesis::GenesisClass;
use katana_primitives::{address, felt};

abigen!(GetChainId, "crates/katana/executor/tests/test-data/get_chain_id.contract.json");

/// Test to ensure that when Blockifier pass the chain id to the contract ( thru a syscall eg,
/// get_tx_inbox().unbox().chain_id ), the value is exactly the same as Katana chain id.
///
/// Issue: <https://github.com/dojoengine/dojo/issues/1595>
#[tokio::test]
#[rstest::rstest]
#[case::mainnet(ChainId::MAINNET)]
#[case::goerli(ChainId::GOERLI)]
#[case::sepolia(ChainId::SEPOLIA)]
#[case::custom_1(ChainId::Id(felt!("0x4b4154414e41")))]
#[case::custom_2(ChainId::Id(felt!("0xc72dd9d5e883e")))]
async fn test_chain_id(#[case] chain_id: ChainId) {
    // prepare test contract
    let json = include_str!("test-data/get_chain_id.contract.json");
    let test_class = read_compiled_class_artifact(json);
    let class_hash = felt!("0x222");
    let address = address!("0x420");

    let mut config = get_default_test_config(SequencingConfig::default());
    config.chain.id = chain_id;
    // declare class
    config.chain.genesis.classes.insert(
        class_hash,
        GenesisClass { sierra: None, casm: Arc::new(test_class), compiled_class_hash: class_hash },
    );

    // deploy the contract
    let contract = GenesisAllocation::Contract(GenesisContractAlloc {
        class_hash: Some(class_hash),
        ..Default::default()
    });
    config.chain.genesis.extend_allocations([(address, contract)]);

    let sequencer = TestSequencer::start(config).await;
    let contract = GetChainIdReader::new(address.into(), sequencer.provider());
    let result = contract.get().call().await.unwrap();

    // Make sure the chain id felt value is the same as the one we start the node with.
    assert_eq!(chain_id.id(), result);
}
