pub mod allocation;
pub mod constant;
pub mod json;

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

#[cfg(feature = "cartridge")]
use account_sdk::artifacts::{Version as ControllerVersion, CONTROLLERS};
use constant::DEFAULT_ACCOUNT_CLASS;
#[cfg(feature = "controller")]
use constant::{CONTROLLER_ACCOUNT_CLASS, CONTROLLER_CLASS_HASH};
use serde::{Deserialize, Serialize};

use self::allocation::{GenesisAccountAlloc, GenesisAllocation, GenesisContractAlloc};
use self::constant::{
    DEFAULT_ACCOUNT_CLASS_HASH, DEFAULT_LEGACY_ERC20_CLASS, DEFAULT_LEGACY_ERC20_CLASS_HASH,
    DEFAULT_LEGACY_UDC_CLASS, DEFAULT_LEGACY_UDC_CLASS_HASH,
};
use crate::block::{BlockHash, BlockNumber, GasPrices};
use crate::class::{ClassHash, ContractClass};
use crate::contract::ContractAddress;
#[cfg(feature = "cartridge")]
use crate::utils::class::parse_sierra_class;
use crate::Felt;

/// Genesis block configuration.
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
    pub classes: BTreeMap<ClassHash, Arc<ContractClass>>,
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
        let mut classes = BTreeMap::new();

        classes.extend(BTreeMap::from([
            // Fee token class
            (DEFAULT_LEGACY_ERC20_CLASS_HASH, DEFAULT_LEGACY_ERC20_CLASS.clone().into()),
            // universal depoyer contract class
            (DEFAULT_LEGACY_UDC_CLASS_HASH, DEFAULT_LEGACY_UDC_CLASS.clone().into()),
            // predeployed account class
            (DEFAULT_ACCOUNT_CLASS_HASH, DEFAULT_ACCOUNT_CLASS.clone().into()),
            // This is controller `1.0.4`.
            #[cfg(feature = "controller")]
            (CONTROLLER_CLASS_HASH, CONTROLLER_ACCOUNT_CLASS.clone().into()),
        ]));

        #[cfg(feature = "cartridge")]
        classes.extend(
            // Filter out the `1.0.4` already included and
            // LATEST which is a duplicate of `1.0.9`.
            CONTROLLERS
                .iter()
                .filter(|(v, _)| {
                    **v == ControllerVersion::V1_0_5
                        || **v == ControllerVersion::V1_0_6
                        || **v == ControllerVersion::V1_0_7
                        || **v == ControllerVersion::V1_0_8
                        || **v == ControllerVersion::V1_0_9
                })
                .map(|(_, v)| (v.hash, parse_sierra_class(v.content).unwrap().into())),
        );

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
