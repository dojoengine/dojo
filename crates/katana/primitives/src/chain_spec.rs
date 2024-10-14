use std::collections::BTreeMap;

use alloy_primitives::U256;
use anyhow::Result;
use lazy_static::lazy_static;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet_crypto::Felt;

use crate::block::{Block, Header};
use crate::chain::ChainId;
use crate::class::ClassHash;
use crate::contract::ContractAddress;
use crate::genesis::allocation::{
    DevAllocationsGenerator, GenesisAllocation, GenesisContractAlloc,
};
use crate::genesis::constant::{
    get_fee_token_balance_base_storage_address, DEFAULT_ACCOUNT_CLASS_PUBKEY_STORAGE_SLOT,
    DEFAULT_ETH_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CLASS_HASH, DEFAULT_LEGACY_UDC_CLASS_HASH,
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, DEFAULT_STRK_FEE_TOKEN_ADDRESS, DEFAULT_UDC_ADDRESS,
    ERC20_DECIMAL_STORAGE_SLOT, ERC20_NAME_STORAGE_SLOT, ERC20_SYMBOL_STORAGE_SLOT,
    ERC20_TOTAL_SUPPLY_STORAGE_SLOT,
};
use crate::genesis::Genesis;
use crate::state::StateUpdatesWithDeclaredClasses;
use crate::utils::split_u256;
use crate::version::CURRENT_STARKNET_VERSION;

/// A chain specification.
// TODO: include l1 core contract
#[derive(Debug, Clone)]
pub struct ChainSpec {
    /// The network chain id.
    pub id: ChainId,
    /// The genesis block.
    pub genesis: Genesis,
    /// The chain fee token contract.
    pub fee_contracts: FeeContracts,
}

/// Tokens that can be used for transaction fee payments in the chain. As
/// supported on Starknet.
// TODO: include both l1 and l2 addresses
#[derive(Debug, Clone)]
pub struct FeeContracts {
    /// L2 ETH fee token address. Used for paying pre-V3 transactions.
    pub eth: ContractAddress,
    /// L2 STRK fee token address. Used for paying V3 transactions.
    pub strk: ContractAddress,
}

impl ChainSpec {
    pub fn genesis_header(&self) -> Header {
        Header {
            version: CURRENT_STARKNET_VERSION,
            number: self.genesis.number,
            timestamp: self.genesis.timestamp,
            state_root: self.genesis.state_root,
            parent_hash: self.genesis.parent_hash,
            gas_prices: self.genesis.gas_prices.clone(),
            sequencer_address: self.genesis.sequencer_address,
        }
    }

    pub fn block(&self) -> Block {
        let header = self.genesis_header();
        Block { header, body: Vec::new() }
    }

    pub fn state_updates(&self) -> Result<StateUpdatesWithDeclaredClasses> {
        let mut states = StateUpdatesWithDeclaredClasses::default();

        for (class_hash, class) in &self.genesis.classes {
            let class_hash = *class_hash;

            states.state_updates.declared_classes.insert(class_hash, class.compiled_class_hash);
            states.declared_compiled_classes.insert(class_hash, class.casm.as_ref().clone());

            if let Some(sierra) = &class.sierra {
                states.declared_sierra_classes.insert(class_hash, sierra.as_ref().clone());
            }
        }

        for (address, alloc) in &self.genesis.allocations {
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

        // -- ETH
        Self::apply_fee_token_storage(
            &mut states,
            "Ether",
            "ETH",
            18,
            DEFAULT_ETH_FEE_TOKEN_ADDRESS,
            DEFAULT_LEGACY_ERC20_CLASS_HASH,
            &self.genesis.allocations,
        );

        // -- STRK
        Self::apply_fee_token_storage(
            &mut states,
            "Starknet Token",
            "STRK",
            18,
            DEFAULT_STRK_FEE_TOKEN_ADDRESS,
            DEFAULT_LEGACY_ERC20_CLASS_HASH,
            &self.genesis.allocations,
        );

        Ok(states)
    }

    fn apply_fee_token_storage(
        states: &mut StateUpdatesWithDeclaredClasses,
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
}

impl Default for ChainSpec {
    fn default() -> Self {
        DEV.clone()
    }
}

lazy_static! {
    /// The default chain specification in dev mode.
    pub static ref DEV: ChainSpec = {
        let mut chain_spec = DEV_UNALLOCATED.clone();

        let accounts = DevAllocationsGenerator::new(10)
            .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
            .generate();

        chain_spec.genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
        chain_spec
    };

    /// The default chain specification for dev mode but without any allocations.
    ///
    /// Used when we want to create a chain spec with user defined # of allocations.
    pub static ref DEV_UNALLOCATED: ChainSpec = {
        let id = ChainId::SEPOLIA;
        let mut genesis = Genesis::default();

        let udc = GenesisAllocation::Contract(GenesisContractAlloc {
            class_hash: Some(DEFAULT_LEGACY_UDC_CLASS_HASH),
            ..Default::default()
        });

        // insert udc
        genesis.extend_allocations([(DEFAULT_UDC_ADDRESS, udc)]);

        let fee_contracts = FeeContracts { eth: DEFAULT_ETH_FEE_TOKEN_ADDRESS, strk: DEFAULT_STRK_FEE_TOKEN_ADDRESS };
        ChainSpec { id, genesis, fee_contracts }
    };
}
