pub mod allocation;
pub mod constant;
pub mod json;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::sync::Arc;

use ethers::types::U256;
use serde::{Deserialize, Serialize};
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::utils::cairo_short_string_to_felt;

use self::allocation::{GenesisAccountAlloc, GenesisAllocation, GenesisContractAlloc};
use self::constant::{
    get_fee_token_balance_base_storage_address, DEFAULT_FEE_TOKEN_ADDRESS,
    DEFAULT_LEGACY_ERC20_CONTRACT_CASM, DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
    DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH, DEFAULT_LEGACY_UDC_CASM,
    DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
    DEFAULT_OZ_ACCOUNT_CONTRACT, DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
    DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
    DEFAULT_UDC_ADDRESS, ERC20_DECIMAL_STORAGE_SLOT, ERC20_NAME_STORAGE_SLOT,
    ERC20_SYMBOL_STORAGE_SLOT, ERC20_TOTAL_SUPPLY_STORAGE_SLOT,
    OZ_ACCOUNT_CONTRACT_PUBKEY_STORAGE_SLOT,
};
use crate::block::{Block, BlockHash, BlockNumber, GasPrices, Header};
use crate::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, FlattenedSierraClass, StorageKey,
    StorageValue,
};
use crate::state::StateUpdatesWithDeclaredClasses;
use crate::utils::split_u256;
use crate::version::CURRENT_STARKNET_VERSION;
use crate::FieldElement;

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeTokenConfig {
    /// The name of the fee token.
    pub name: String,
    /// The symbol of the fee token.
    pub symbol: String,
    /// The address of the fee token contract.
    pub address: ContractAddress,
    /// The decimals of the fee token.
    pub decimals: u8,
    /// The total supply of the fee token.
    pub total_supply: U256,
    /// The class hash of the fee token contract.
    #[serde_as(as = "UfeHex")]
    pub class_hash: ClassHash,
    /// To initialize the fee token contract storage
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GenesisClass {
    /// The compiled class hash of the contract class.
    #[serde_as(as = "UfeHex")]
    pub compiled_class_hash: CompiledClassHash,
    /// The casm class definition.
    #[serde(skip_serializing)]
    pub casm: Arc<CompiledClass>,
    /// The sierra class definition.
    #[serde(skip_serializing)]
    pub sierra: Option<Arc<FlattenedSierraClass>>,
}

/// The configuration of the universal deployer contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UniversalDeployerConfig {
    /// The class hash of the universal deployer contract.
    pub class_hash: ClassHash,
    /// The address of the universal deployer contract.
    pub address: ContractAddress,
    /// To initialize the UD contract storage
    pub storage: Option<HashMap<StorageKey, StorageValue>>,
}

/// Genesis block configuration.
#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize)]
pub struct Genesis {
    /// The genesis block parent hash.
    #[serde_as(as = "UfeHex")]
    pub parent_hash: BlockHash,
    /// The genesis block state root.
    #[serde_as(as = "UfeHex")]
    pub state_root: FieldElement,
    /// The genesis block number.
    pub number: BlockNumber,
    /// The genesis block timestamp.
    pub timestamp: u64,
    /// The genesis block sequencer address.
    pub sequencer_address: ContractAddress,
    /// The genesis block L1 gas prices.
    pub gas_prices: GasPrices,
    /// The classes to declare in the genesis block.
    pub classes: HashMap<ClassHash, GenesisClass>,
    /// The fee token configuration.
    pub fee_token: FeeTokenConfig,
    /// The universal deployer (UDC) configuration.
    pub universal_deployer: Option<UniversalDeployerConfig>,
    /// The genesis contract allocations.
    pub allocations: BTreeMap<ContractAddress, GenesisAllocation>,
}

impl Genesis {
    /// Extends the genesis allocations with the given allocations.
    pub fn extend_allocations<T>(&mut self, allocs: T)
    where
        T: IntoIterator<Item = (ContractAddress, GenesisAllocation)>,
    {
        self.allocations.extend(allocs);
    }

    /// Returns an iterator over the generic (non-account) contracts.
    pub fn contracts(&self) -> impl Iterator<Item = &GenesisContractAlloc> {
        self.allocations.values().filter_map(|allocation| {
            if let GenesisAllocation::Contract(contract) = allocation {
                Some(contract)
            } else {
                None
            }
        })
    }

    /// Returns an iterator over the genesis accounts. This will only return
    /// allocated account contracts.
    pub fn accounts(&self) -> impl Iterator<Item = (&ContractAddress, &GenesisAccountAlloc)> {
        self.allocations.iter().filter_map(|(addr, alloc)| {
            if let GenesisAllocation::Account(account) = alloc {
                Some((addr, account))
            } else {
                None
            }
        })
    }

    /// Get the genesis in the form of a block.
    pub fn block(&self) -> Block {
        Block {
            header: Header {
                parent_hash: self.parent_hash,
                number: self.number,
                state_root: self.state_root,
                timestamp: self.timestamp,
                gas_prices: self.gas_prices,
                sequencer_address: self.sequencer_address,
                version: CURRENT_STARKNET_VERSION,
            },
            body: Vec::new(),
        }
    }

    /// Get the genesis in the form of state updates.
    pub fn state_updates(&self) -> StateUpdatesWithDeclaredClasses {
        let mut states = StateUpdatesWithDeclaredClasses::default();

        for (class_hash, class) in &self.classes {
            let class_hash = *class_hash;

            states.state_updates.declared_classes.insert(class_hash, class.compiled_class_hash);
            states.declared_compiled_classes.insert(class_hash, class.casm.as_ref().clone());

            if let Some(sierra) = &class.sierra {
                states.declared_sierra_classes.insert(class_hash, sierra.as_ref().clone());
            }
        }

        for (address, alloc) in &self.allocations {
            let address = *address;

            if let Some(hash) = alloc.class_hash() {
                states.state_updates.contract_updates.insert(address, hash);
            }

            if let Some(nonce) = alloc.nonce() {
                states.state_updates.nonce_updates.insert(address, nonce);
            }

            let mut storage = alloc.storage().cloned().unwrap_or_default();
            if let Some(pub_key) = alloc.public_key() {
                storage.insert(OZ_ACCOUNT_CONTRACT_PUBKEY_STORAGE_SLOT, pub_key);
            }

            states.state_updates.storage_updates.insert(address, storage);
        }

        // TODO: put this in a separate function

        // insert fee token related data
        let mut fee_token_storage = self.fee_token.storage.clone().unwrap_or_default();

        let name: FieldElement = cairo_short_string_to_felt(&self.fee_token.name).unwrap();
        let symbol: FieldElement = cairo_short_string_to_felt(&self.fee_token.symbol).unwrap();
        let decimals: FieldElement = self.fee_token.decimals.into();
        let (total_supply_low, total_supply_high) = split_u256(self.fee_token.total_supply);

        fee_token_storage.insert(ERC20_NAME_STORAGE_SLOT, name);
        fee_token_storage.insert(ERC20_SYMBOL_STORAGE_SLOT, symbol);
        fee_token_storage.insert(ERC20_DECIMAL_STORAGE_SLOT, decimals);
        fee_token_storage.insert(ERC20_TOTAL_SUPPLY_STORAGE_SLOT, total_supply_low);
        fee_token_storage.insert(ERC20_TOTAL_SUPPLY_STORAGE_SLOT + 1u8.into(), total_supply_high);

        for (address, alloc) in &self.allocations {
            if let Some(balance) = alloc.balance() {
                let (low, high) = split_u256(balance);

                // the base storage address for a standard ERC20 contract balance
                let bal_base_storage_var = get_fee_token_balance_base_storage_address(*address);

                // the storage address of low u128 of the balance
                let low_bal_storage_var = bal_base_storage_var;
                // the storage address of high u128 of the balance
                let high_bal_storage_var = bal_base_storage_var + 1u8.into();

                fee_token_storage.insert(low_bal_storage_var, low);
                fee_token_storage.insert(high_bal_storage_var, high);
            }
        }

        states
            .state_updates
            .contract_updates
            .insert(self.fee_token.address, self.fee_token.class_hash);
        states.state_updates.storage_updates.insert(self.fee_token.address, fee_token_storage);

        // insert universal deployer related data
        if let Some(udc) = &self.universal_deployer {
            let storage = udc.storage.clone().unwrap_or_default();

            states.state_updates.contract_updates.insert(udc.address, udc.class_hash);
            states.state_updates.storage_updates.insert(udc.address, storage);
        }

        states
    }
}

impl Default for Genesis {
    /// Creates a new [Genesis] with the default configurations and classes. The default
    /// classes are a legacy ERC20 class for the fee token, a legacy UDC class for the
    /// universal deployer, and an OpenZeppelin account contract class.
    fn default() -> Self {
        let fee_token = FeeTokenConfig {
            decimals: 18,
            name: "Ether".into(),
            symbol: "ETH".into(),
            total_supply: 0.into(),
            address: DEFAULT_FEE_TOKEN_ADDRESS,
            class_hash: DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
            storage: None,
        };

        let universal_deployer = UniversalDeployerConfig {
            address: DEFAULT_UDC_ADDRESS,
            class_hash: DEFAULT_LEGACY_UDC_CLASS_HASH,
            storage: None,
        };

        let classes = HashMap::from([
            (
                DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH,
                },
            ),
            (
                DEFAULT_LEGACY_UDC_CLASS_HASH,
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_UDC_CASM.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
                },
            ),
            (
                DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                GenesisClass {
                    sierra: Some(DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap().into()),
                    casm: DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone().into(),
                    compiled_class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
                },
            ),
        ]);

        Self {
            parent_hash: FieldElement::ZERO,
            number: 0,
            state_root: FieldElement::ZERO,
            timestamp: 0,
            gas_prices: GasPrices::default(),
            sequencer_address: FieldElement::ZERO.into(),
            classes,
            allocations: BTreeMap::new(),
            fee_token,
            universal_deployer: Some(universal_deployer),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use starknet::macros::felt;
    use tests::allocation::GenesisAccount;
    use tests::constant::{
        DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CONTRACT_CASM,
        DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
        DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH, DEFAULT_LEGACY_UDC_CASM,
        DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
        DEFAULT_OZ_ACCOUNT_CONTRACT, DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
        DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
    };

    use super::*;

    #[test]
    fn genesis_block_and_state_updates() {
        // setup initial states to test

        let classes = HashMap::from([
            (
                DEFAULT_LEGACY_UDC_CLASS_HASH,
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_UDC_CASM.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
                },
            ),
            (
                DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
                GenesisClass {
                    sierra: None,
                    casm: DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH,
                },
            ),
            (
                DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                GenesisClass {
                    compiled_class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH,
                    casm: DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone().into(),
                    sierra: Some(DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap().into()),
                },
            ),
        ]);

        let fee_token = FeeTokenConfig {
            address: DEFAULT_FEE_TOKEN_ADDRESS,
            name: String::from("ETHER"),
            symbol: String::from("ETH"),
            total_supply: U256::from_str("0x1a784379d99db42000000").unwrap(),
            decimals: 18,
            class_hash: DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH,
            storage: Some(HashMap::from([
                (felt!("0x111"), felt!("0x1")),
                (felt!("0x222"), felt!("0x2")),
            ])),
        };

        let allocations = [
            (
                ContractAddress::from(felt!("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")),
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    public_key: felt!(
                        "0x01ef15c18599971b7beced415a40f0c7deacfd9b0d1819e03d723d8bc943cfca"
                    ),
                    balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
                    class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                    nonce: Some(felt!("0x99")),
                    storage: Some(HashMap::from([
                        (felt!("0x1"), felt!("0x1")),
                        (felt!("0x2"), felt!("0x2")),
                    ])),
                })),
            ),
            (
                ContractAddress::from(felt!("0xdeadbeef")),
                GenesisAllocation::Contract(GenesisContractAlloc {
                    balance: Some(U256::from_str("0xD3C21BCECCEDA1000000").unwrap()),
                    class_hash: Some(DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH),
                    nonce: Some(felt!("0x100")),
                    storage: Some(HashMap::from([
                        (felt!("0x100"), felt!("0x111")),
                        (felt!("0x200"), felt!("0x222")),
                    ])),
                }),
            ),
            (
                ContractAddress::from(felt!("0x2")),
                GenesisAllocation::Account(GenesisAccountAlloc::Account(GenesisAccount {
                    public_key: felt!("0x2"),
                    balance: Some(U256::zero()),
                    class_hash: DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
                    nonce: None,
                    storage: None,
                })),
            ),
        ];

        let ud = UniversalDeployerConfig {
            address: ContractAddress(felt!("0xb00b1e5")),
            class_hash: DEFAULT_LEGACY_UDC_CLASS_HASH,
            storage: Some([(felt!("0x10"), felt!("0x100"))].into()),
        };

        let genesis = Genesis {
            classes,
            fee_token: fee_token.clone(),
            allocations: BTreeMap::from(allocations.clone()),
            number: 0,
            timestamp: 5123512314u64,
            state_root: felt!("0x99"),
            parent_hash: felt!("0x999"),
            sequencer_address: ContractAddress(felt!("0x100")),
            gas_prices: GasPrices { eth: 1111, strk: 2222 },
            universal_deployer: Some(ud.clone()),
        };

        // setup expected values

        let name: FieldElement = cairo_short_string_to_felt(&fee_token.name).unwrap();
        let symbol: FieldElement = cairo_short_string_to_felt(&fee_token.symbol).unwrap();
        let decimals: FieldElement = fee_token.decimals.into();
        let (total_supply_low, total_supply_high) = split_u256(fee_token.total_supply);

        let mut fee_token_storage = HashMap::new();
        fee_token_storage.insert(ERC20_NAME_STORAGE_SLOT, name);
        fee_token_storage.insert(ERC20_SYMBOL_STORAGE_SLOT, symbol);
        fee_token_storage.insert(ERC20_DECIMAL_STORAGE_SLOT, decimals);
        fee_token_storage.insert(ERC20_TOTAL_SUPPLY_STORAGE_SLOT, total_supply_low);
        fee_token_storage.insert(ERC20_TOTAL_SUPPLY_STORAGE_SLOT + 1u8.into(), total_supply_high);

        for (address, alloc) in &allocations {
            if let Some(balance) = alloc.balance() {
                let (low, high) = split_u256(balance);

                // the base storage address for a standard ERC20 contract balance
                let bal_base_storage_var = get_fee_token_balance_base_storage_address(*address);

                // the storage address of low u128 of the balance
                let low_bal_storage_var = bal_base_storage_var;
                // the storage address of high u128 of the balance
                let high_bal_storage_var = bal_base_storage_var + 1u8.into();

                fee_token_storage.insert(low_bal_storage_var, low);
                fee_token_storage.insert(high_bal_storage_var, high);
            }
        }

        let expected_block = Block {
            header: Header {
                number: genesis.number,
                timestamp: genesis.timestamp,
                state_root: genesis.state_root,
                parent_hash: genesis.parent_hash,
                sequencer_address: genesis.sequencer_address,
                gas_prices: genesis.gas_prices,
                version: CURRENT_STARKNET_VERSION,
            },
            body: Vec::new(),
        };

        let actual_block = genesis.block();
        let actual_state_updates = genesis.state_updates();

        // assert individual fields of the block

        assert_eq!(actual_block.header.number, expected_block.header.number);
        assert_eq!(actual_block.header.timestamp, expected_block.header.timestamp);
        assert_eq!(actual_block.header.state_root, expected_block.header.state_root);
        assert_eq!(actual_block.header.parent_hash, expected_block.header.parent_hash);
        assert_eq!(actual_block.header.sequencer_address, expected_block.header.sequencer_address);
        assert_eq!(actual_block.header.gas_prices, expected_block.header.gas_prices);
        assert_eq!(actual_block.header.version, expected_block.header.version);
        assert_eq!(actual_block.body, expected_block.body);

        assert!(
            actual_state_updates.declared_compiled_classes.len() == 3,
            "should be 3 casm classes: udc, erc20, oz account"
        );
        assert!(
            actual_state_updates.declared_sierra_classes.len() == 1,
            "should be only 1 sierra class: oz account"
        );

        assert_eq!(
            actual_state_updates.state_updates.declared_classes.get(&fee_token.class_hash),
            Some(&DEFAULT_LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH),
            "The default fee token class should be declared"
        );

        assert_eq!(
            actual_state_updates.declared_compiled_classes.get(&fee_token.class_hash),
            Some(&DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone()),
            "The default fee token casm class should be declared"
        );

        assert!(
            actual_state_updates.declared_sierra_classes.get(&fee_token.class_hash).is_none(),
            "The default fee token class doesnt have a sierra class"
        );

        assert_eq!(
            actual_state_updates.state_updates.contract_updates.get(&fee_token.address),
            Some(&fee_token.class_hash),
            "The fee token contract should be created"
        );

        assert_eq!(
            actual_state_updates.state_updates.declared_classes.get(&ud.class_hash),
            Some(&DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH),
            "The default universal deployer class should be declared"
        );

        assert_eq!(
            actual_state_updates.declared_compiled_classes.get(&ud.class_hash),
            Some(&DEFAULT_LEGACY_UDC_CASM.clone()),
            "The default universal deployer casm class should be declared"
        );

        assert!(
            actual_state_updates.declared_sierra_classes.get(&ud.class_hash).is_none(),
            "The default universal deployer class doesnt have a sierra class"
        );

        assert_eq!(
            actual_state_updates.state_updates.contract_updates.get(&ud.address),
            Some(&ud.class_hash),
            "The universal deployer contract should be created"
        );

        assert_eq!(
            actual_state_updates
                .state_updates
                .declared_classes
                .get(&DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH),
            Some(&DEFAULT_OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH),
            "The default oz account class should be declared"
        );

        assert_eq!(
            actual_state_updates
                .declared_compiled_classes
                .get(&DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH)
                .unwrap(),
            &DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone(),
            "The default oz account contract casm class should be declared"
        );

        assert_eq!(
            actual_state_updates
                .declared_sierra_classes
                .get(&DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH),
            Some(&DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap()),
            "The default oz account contract sierra class should be declared"
        );

        // check that all contract allocations exist in the state updates

        assert_eq!(
            actual_state_updates.state_updates.contract_updates.len(),
            5,
            "5 contracts should be created: fee token, universal deployer, and 3 allocations"
        );

        let alloc_1_addr = allocations[0].0;

        let mut account_allocation_storage = allocations[0].1.storage().unwrap().clone();
        account_allocation_storage.insert(
            OZ_ACCOUNT_CONTRACT_PUBKEY_STORAGE_SLOT,
            felt!("0x01ef15c18599971b7beced415a40f0c7deacfd9b0d1819e03d723d8bc943cfca"),
        );

        assert_eq!(
            actual_state_updates.state_updates.contract_updates.get(&alloc_1_addr),
            allocations[0].1.class_hash().as_ref(),
            "allocation should exist"
        );
        assert_eq!(
            actual_state_updates.state_updates.nonce_updates.get(&alloc_1_addr).cloned(),
            allocations[0].1.nonce(),
            "allocation nonce should be updated"
        );
        assert_eq!(
            actual_state_updates.state_updates.storage_updates.get(&alloc_1_addr).cloned(),
            Some(account_allocation_storage),
            "account allocation storage should be updated"
        );

        let alloc_2_addr = allocations[1].0;

        assert_eq!(
            actual_state_updates.state_updates.contract_updates.get(&alloc_2_addr),
            allocations[1].1.class_hash().as_ref(),
            "allocation should exist"
        );
        assert_eq!(
            actual_state_updates.state_updates.nonce_updates.get(&alloc_2_addr).cloned(),
            allocations[1].1.nonce(),
            "allocation nonce should be updated"
        );
        assert_eq!(
            actual_state_updates.state_updates.storage_updates.get(&alloc_2_addr),
            allocations[1].1.storage(),
            "allocation storage should be updated"
        );

        let alloc_3_addr = allocations[2].0;

        assert_eq!(
            actual_state_updates.state_updates.contract_updates.get(&alloc_3_addr),
            allocations[2].1.class_hash().as_ref(),
            "allocation should exist"
        );
        assert_eq!(
            actual_state_updates.state_updates.nonce_updates.get(&alloc_3_addr).cloned(),
            allocations[2].1.nonce(),
            "allocation nonce should be updated"
        );
        assert_eq!(
            actual_state_updates.state_updates.storage_updates.get(&alloc_3_addr).cloned(),
            Some(HashMap::from([(OZ_ACCOUNT_CONTRACT_PUBKEY_STORAGE_SLOT, felt!("0x2"))])),
            "account allocation storage should be updated"
        );

        // check fee token contract storage

        let fee_token_storage =
            actual_state_updates.state_updates.storage_updates.get(&fee_token.address).unwrap();

        assert_eq!(fee_token_storage.get(&ERC20_NAME_STORAGE_SLOT), Some(&name));
        assert_eq!(fee_token_storage.get(&ERC20_SYMBOL_STORAGE_SLOT), Some(&symbol));
        assert_eq!(fee_token_storage.get(&ERC20_DECIMAL_STORAGE_SLOT), Some(&decimals));
        assert_eq!(
            fee_token_storage.get(&ERC20_TOTAL_SUPPLY_STORAGE_SLOT),
            Some(&total_supply_low)
        );
        assert_eq!(
            fee_token_storage.get(&(ERC20_TOTAL_SUPPLY_STORAGE_SLOT + 1u8.into())),
            Some(&total_supply_high)
        );

        // check generic non-fee token specific storage

        assert_eq!(fee_token_storage.get(&felt!("0x111")), Some(&felt!("0x1")));
        assert_eq!(fee_token_storage.get(&felt!("0x222")), Some(&felt!("0x2")));

        let mut actual_total_supply = U256::zero();

        // check for balance
        for (address, alloc) in &allocations {
            if let Some(balance) = alloc.balance() {
                let (low, high) = split_u256(balance);

                // the base storage address for a standard ERC20 contract balance
                let bal_base_storage_var = get_fee_token_balance_base_storage_address(*address);

                // the storage address of low u128 of the balance
                let low_bal_storage_var = bal_base_storage_var;
                // the storage address of high u128 of the balance
                let high_bal_storage_var = bal_base_storage_var + 1u8.into();

                assert_eq!(fee_token_storage.get(&low_bal_storage_var), Some(&low));
                assert_eq!(fee_token_storage.get(&high_bal_storage_var), Some(&high));

                actual_total_supply += balance;
            }
        }

        assert_eq!(
            actual_total_supply, fee_token.total_supply,
            "total supply should match the total balances of all allocations"
        );

        let udc_storage =
            actual_state_updates.state_updates.storage_updates.get(&ud.address).unwrap();

        // check universal deployer contract storage

        assert_eq!(udc_storage.get(&felt!("0x10")), Some(&felt!("0x100")));
    }
}
