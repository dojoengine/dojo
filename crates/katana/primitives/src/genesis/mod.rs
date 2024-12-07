pub mod allocation;
pub mod constant;
pub mod json;

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use constant::DEFAULT_ACCOUNT_CLASS;
#[cfg(feature = "slot")]
use constant::{CONTROLLER_ACCOUNT_CLASS, CONTROLLER_CLASS_HASH};
use serde::{Deserialize, Serialize};

use self::allocation::{GenesisAccountAlloc, GenesisAllocation, GenesisContractAlloc};
use self::constant::{
    DEFAULT_ACCOUNT_CLASS_HASH, DEFAULT_ACCOUNT_COMPILED_CLASS_HASH, DEFAULT_LEGACY_ERC20_CLASS,
    DEFAULT_LEGACY_ERC20_CLASS_HASH, DEFAULT_LEGACY_ERC20_COMPILED_CLASS_HASH,
    DEFAULT_LEGACY_UDC_CLASS, DEFAULT_LEGACY_UDC_CLASS_HASH,
    DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
};
use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::class::{ClassHash, CompiledClassHash, ContractClass};
use crate::contract::ContractAddress;
use crate::Felt;

#[derive(Clone, Serialize, PartialEq, Eq, Deserialize)]
pub struct GenesisClass {
    /// The compiled class hash of the contract class.
    pub compiled_class_hash: CompiledClassHash,
    pub class: Arc<ContractClass>,
}

impl core::fmt::Debug for GenesisClass {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenesisClass")
            .field("compiled_class_hash", &self.compiled_class_hash)
            .field("class", &"...")
            .finish()
    }
}

/// Genesis block configuration.
#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Genesis {
    /// The genesis block parent hash.
    pub parent_hash: BlockHash,
    /// The genesis block state root.
    pub state_root: Felt,
    /// The genesis block number.
    pub number: BlockNumber,
    /// The genesis block timestamp.
    pub timestamp: u64,
    /// The genesis block sequencer address.
    pub sequencer_address: ContractAddress,
    /// The genesis block L1 gas prices.
    pub gas_prices: GasPrices,
    /// The classes to declare in the genesis block.
    pub classes: BTreeMap<ClassHash, GenesisClass>,
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
}

impl Default for Genesis {
    /// Creates a new [Genesis] with the default configurations and classes. The default
    /// classes are a legacy ERC20 class for the fee token, a legacy UDC class for the
    /// universal deployer, and an OpenZeppelin account contract class.
    fn default() -> Self {
        let classes = BTreeMap::from([
            // Fee token class
            (
                DEFAULT_LEGACY_ERC20_CLASS_HASH,
                GenesisClass {
                    class: DEFAULT_LEGACY_ERC20_CLASS.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_ERC20_COMPILED_CLASS_HASH,
                },
            ),
            // universal depoyer contract class
            (
                DEFAULT_LEGACY_UDC_CLASS_HASH,
                GenesisClass {
                    class: DEFAULT_LEGACY_UDC_CLASS.clone().into(),
                    compiled_class_hash: DEFAULT_LEGACY_UDC_COMPILED_CLASS_HASH,
                },
            ),
            // predeployed account class
            (
                DEFAULT_ACCOUNT_CLASS_HASH,
                GenesisClass {
                    compiled_class_hash: DEFAULT_ACCOUNT_COMPILED_CLASS_HASH,
                    class: DEFAULT_ACCOUNT_CLASS.clone().into(),
                },
            ),
            #[cfg(feature = "slot")]
            (
                CONTROLLER_CLASS_HASH,
                GenesisClass {
                    compiled_class_hash: CONTROLLER_CLASS_HASH,
                    class: CONTROLLER_ACCOUNT_CLASS.clone().into(),
                },
            ),
        ]);

        Self {
            parent_hash: Felt::ZERO,
            number: 0,
            state_root: Felt::ZERO,
            timestamp: 0,
            gas_prices: GasPrices::default(),
            sequencer_address: Felt::ZERO.into(),
            classes,
            allocations: BTreeMap::new(),
        }
    }
}
