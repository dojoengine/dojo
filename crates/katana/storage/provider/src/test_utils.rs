use std::collections::BTreeMap;
use std::sync::Arc;

use alloy_primitives::U256;
use katana_chain_spec::ChainSpec;
use katana_db::mdbx::test_utils;
use katana_primitives::block::{Block, BlockHash, FinalityStatus};
use katana_primitives::class::ClassHash;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{
    DevGenesisAccount, GenesisAccountAlloc, GenesisAllocation,
};
use katana_primitives::genesis::constant::{
    get_fee_token_balance_base_storage_address, DEFAULT_ACCOUNT_CLASS_PUBKEY_STORAGE_SLOT,
    DEFAULT_ETH_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CLASS, DEFAULT_LEGACY_ERC20_CLASS_HASH,
    DEFAULT_LEGACY_UDC_CLASS, DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_STRK_FEE_TOKEN_ADDRESS,
    DEFAULT_UDC_ADDRESS, ERC20_DECIMAL_STORAGE_SLOT, ERC20_NAME_STORAGE_SLOT,
    ERC20_SYMBOL_STORAGE_SLOT, ERC20_TOTAL_SUPPLY_STORAGE_SLOT,
};
use katana_primitives::genesis::Genesis;
use katana_primitives::state::StateUpdatesWithClasses;
use katana_primitives::utils::class::parse_sierra_class;
use katana_primitives::utils::split_u256;
use katana_primitives::{address, Felt};
use starknet::core::utils::cairo_short_string_to_felt;
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
    let state_updates = get_state_updates(&chain.genesis);
    let block = Block::default().seal_with_hash_and_status(hash, status);

    provider
        .insert_block_with_states_and_receipts(block, state_updates, Vec::new(), Vec::new())
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
        Arc::new(class)
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

// This is a placeholder.
pub fn get_state_updates(genesis: &Genesis) -> StateUpdatesWithClasses {
    let mut states = StateUpdatesWithClasses::default();

    for (class_hash, class) in &genesis.classes {
        let class_hash = *class_hash;

        if class.is_legacy() {
            states.state_updates.deprecated_declared_classes.insert(class_hash);
        } else {
            let compiled_hash = class.as_ref().clone().compile().unwrap().class_hash().unwrap();
            states.state_updates.declared_classes.insert(class_hash, compiled_hash);
        }

        states.classes.insert(class_hash, class.as_ref().clone());
    }

    for (address, alloc) in &genesis.allocations {
        let address = *address;

        if let Some(hash) = alloc.class_hash() {
            states.state_updates.deployed_contracts.insert(address, hash);
        }

        if let Some(nonce) = alloc.nonce() {
            states.state_updates.nonce_updates.insert(address, nonce);
        }

        let mut storage = alloc.storage().cloned().unwrap_or_default();
        if let Some(pub_key) = alloc.public_key() {
            storage.insert(DEFAULT_ACCOUNT_CLASS_PUBKEY_STORAGE_SLOT, pub_key);
        }

        states.state_updates.storage_updates.insert(address, storage);
    }

    //-- Fee tokens
    add_default_fee_tokens(&mut states, genesis);
    // -- UDC
    add_default_udc(&mut states);

    states
}

fn add_default_fee_tokens(states: &mut StateUpdatesWithClasses, genesis: &Genesis) {
    // declare erc20 token contract
    states
        .classes
        .entry(DEFAULT_LEGACY_ERC20_CLASS_HASH)
        .or_insert_with(|| DEFAULT_LEGACY_ERC20_CLASS.clone());

    // -- ETH
    add_fee_token(
        states,
        "Ether",
        "ETH",
        18,
        DEFAULT_ETH_FEE_TOKEN_ADDRESS,
        DEFAULT_LEGACY_ERC20_CLASS_HASH,
        &genesis.allocations,
    );

    // -- STRK
    add_fee_token(
        states,
        "Starknet Token",
        "STRK",
        18,
        DEFAULT_STRK_FEE_TOKEN_ADDRESS,
        DEFAULT_LEGACY_ERC20_CLASS_HASH,
        &genesis.allocations,
    );
}

fn add_fee_token(
    states: &mut StateUpdatesWithClasses,
    name: &str,
    symbol: &str,
    decimals: u8,
    address: ContractAddress,
    class_hash: ClassHash,
    allocations: &BTreeMap<ContractAddress, GenesisAllocation>,
) {
    let mut storage = BTreeMap::new();
    let mut total_supply = U256::ZERO;

    // --- set the ERC20 balances for each allocations that have a balance

    for (address, alloc) in allocations {
        if let Some(balance) = alloc.balance() {
            total_supply += balance;
            let (low, high) = split_u256(balance);

            // the base storage address for a standard ERC20 contract balance
            let bal_base_storage_var = get_fee_token_balance_base_storage_address(*address);

            // the storage address of low u128 of the balance
            let low_bal_storage_var = bal_base_storage_var;
            // the storage address of high u128 of the balance
            let high_bal_storage_var = bal_base_storage_var + Felt::ONE;

            storage.insert(low_bal_storage_var, low);
            storage.insert(high_bal_storage_var, high);
        }
    }

    // --- ERC20 metadata

    let name = cairo_short_string_to_felt(name).unwrap();
    let symbol = cairo_short_string_to_felt(symbol).unwrap();
    let decimals = decimals.into();
    let (total_supply_low, total_supply_high) = split_u256(total_supply);

    storage.insert(ERC20_NAME_STORAGE_SLOT, name);
    storage.insert(ERC20_SYMBOL_STORAGE_SLOT, symbol);
    storage.insert(ERC20_DECIMAL_STORAGE_SLOT, decimals);
    storage.insert(ERC20_TOTAL_SUPPLY_STORAGE_SLOT, total_supply_low);
    storage.insert(ERC20_TOTAL_SUPPLY_STORAGE_SLOT + Felt::ONE, total_supply_high);

    states.state_updates.deployed_contracts.insert(address, class_hash);
    states.state_updates.storage_updates.insert(address, storage);
}

fn add_default_udc(states: &mut StateUpdatesWithClasses) {
    // declare UDC class
    states
        .classes
        .entry(DEFAULT_LEGACY_UDC_CLASS_HASH)
        .or_insert_with(|| DEFAULT_LEGACY_UDC_CLASS.clone());

    states.state_updates.deprecated_declared_classes.insert(DEFAULT_LEGACY_UDC_CLASS_HASH);

    // deploy UDC contract
    states
        .state_updates
        .deployed_contracts
        .entry(DEFAULT_UDC_ADDRESS)
        .or_insert(DEFAULT_LEGACY_UDC_CLASS_HASH);
}
