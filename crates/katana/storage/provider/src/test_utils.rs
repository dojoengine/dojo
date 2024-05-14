use crate::providers::db::DbProvider;
use crate::{providers::in_memory::InMemoryProvider, traits::block::BlockWriter};

use katana_db::mdbx::{test_utils, DbEnvKind};
use katana_primitives::block::{BlockHash, FinalityStatus};
use katana_primitives::genesis::allocation::{
    DevGenesisAccount, GenesisAccountAlloc, GenesisAllocation,
};
use katana_primitives::genesis::json::parse_genesis_class_artifacts;
use katana_primitives::genesis::{Genesis, GenesisBuilder};
use starknet::macros::felt;

const GENESIS_BUILD_ERROR: &str = "Failed to build genesis block.";
const GENESIS_INIT_ERROR: &str =
    "Failed to initialize test provider with genesis block and states.";

/// Creates an in-memory provider with initial states loaded for testing.
pub fn test_in_memory_provider() -> InMemoryProvider {
    let provider = InMemoryProvider::new();
    initialize_test_provider(&provider);
    provider
}

/// Creates a persistent storage provider with initial states loaded for testin.
pub fn test_db_provider() -> DbProvider {
    let provider = DbProvider::new(test_utils::create_test_db(DbEnvKind::RW));
    initialize_test_provider(&provider);
    provider
}

/// Initializes the provider with a genesis block and states.
fn initialize_test_provider<P: BlockWriter>(provider: &P) {
    let genesis = create_genesis_for_testing();

    let hash = BlockHash::ZERO;
    let status = FinalityStatus::AcceptedOnL2;
    let block = genesis.block().seal_with_hash_and_status(hash, status);
    let states = genesis.state_updates();

    provider
        .insert_block_with_states_and_receipts(block, states, Vec::new(), Vec::new())
        .expect(GENESIS_INIT_ERROR);
}

/// Creates a genesis config specifically for testing purposes.
/// This includes:
/// - An account with simple `__execute__` function, deployed at address `0x1`.
pub fn create_genesis_for_testing() -> Genesis {
    let json = include_str!("../test-data/simple_account.sierra.json");
    let json = serde_json::from_str(json).unwrap();
    let (hash, class) = parse_genesis_class_artifacts(Some(felt!("0x2")), json).unwrap();

    let (_, account) = DevGenesisAccount::new(felt!("0x1"), hash);
    let account = GenesisAllocation::Account(GenesisAccountAlloc::DevAccount(account));

    let mut genesis = Genesis::default();
    genesis.classes.insert(hash, class); // insert the test class
    genesis.extend_allocations([(felt!("0x1"), account)]); // insert the test account
    genesis
}
