use std::sync::Arc;

use alloy_primitives::U256;
use katana_chain_spec::ChainSpec;
use katana_db::mdbx::test_utils;
use katana_primitives::address;
use katana_primitives::block::{BlockHash, FinalityStatus};
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{
    DevGenesisAccount, GenesisAccountAlloc, GenesisAllocation,
};
use katana_primitives::genesis::{Genesis, GenesisClass};
use katana_primitives::utils::class::parse_sierra_class;
use starknet::macros::felt;

use crate::providers::db::DbProvider;
use crate::traits::block::BlockWriter;

/// Creates a persistent storage provider with initial states loaded for testin.
pub fn test_provider() -> DbProvider {
    let provider = DbProvider::new(test_utils::create_test_db());
    initialize_test_provider(&provider);
    provider
}

/// Initializes the provider with a genesis block and states.
fn initialize_test_provider<P: BlockWriter>(provider: &P) {
    let chain = create_chain_for_testing();

    let hash = BlockHash::ZERO;
    let status = FinalityStatus::AcceptedOnL2;
    let block = chain.block().seal_with_hash_and_status(hash, status);
    let states = chain.state_updates();

    provider
        .insert_block_with_states_and_receipts(block, states, Vec::new(), Vec::new())
        .expect("Failed to initialize test provider with genesis block and states.");
}

/// Creates a genesis config specifically for testing purposes.
/// This includes:
/// - An account with simple `__execute__` function, deployed at address `0x1`.
pub fn create_chain_for_testing() -> ChainSpec {
    let mut chain = katana_chain_spec::DEV_UNALLOCATED.clone();

    let class_hash = felt!("0x111");
    let address = address!("0x1");

    // TODO: we should have a genesis builder that can do all of this for us.
    let class = {
        let json = include_str!("../test-data/simple_account.sierra.json");
        let class = parse_sierra_class(json).unwrap();
        GenesisClass { compiled_class_hash: class_hash, class: Arc::new(class) }
    };

    // setup test account
    let (_, account) = DevGenesisAccount::new_with_balance(felt!("0x1"), class_hash, U256::MAX);
    let account = GenesisAllocation::Account(GenesisAccountAlloc::DevAccount(account));

    let mut genesis = Genesis::default();
    // insert test account class and contract
    genesis.classes.insert(class_hash, class);
    genesis.extend_allocations([(address, account)]);

    chain.genesis = genesis;
    chain
}
