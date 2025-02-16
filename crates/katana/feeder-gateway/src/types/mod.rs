use std::collections::{BTreeMap, BTreeSet};

use katana_primitives::block::{BlockHash, BlockNumber};
pub use katana_primitives::class::CasmContractClass;
use katana_primitives::class::{
    ClassHash, CompiledClassHash, LegacyContractClass, SierraContractClass,
};
use katana_primitives::contract::{Nonce, StorageKey, StorageValue};
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::version::ProtocolVersion;
use katana_primitives::{ContractAddress, Felt};
use katana_rpc_types::class::ConversionError;
pub use katana_rpc_types::class::RpcSierraContractClass;
use serde::Deserialize;
use starknet::core::types::ResourcePrice;
use starknet::providers::sequencer::models::BlockStatus;

mod receipt;
mod serde_utils;
mod transaction;

pub use receipt::*;
pub use transaction::*;

/// The contract class type returns by `/get_class_by_hash` endpoint.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ContractClass {
    Class(RpcSierraContractClass),
    Legacy(LegacyContractClass),
}

/// The state update type returns by `/get_state_update` endpoint.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct StateUpdate {
    pub block_hash: Option<Felt>,
    pub new_root: Option<Felt>,
    pub old_root: Felt,
    pub state_diff: StateDiff,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct StateDiff {
    pub storage_diffs: BTreeMap<ContractAddress, Vec<StorageDiff>>,
    pub deployed_contracts: Vec<DeployedContract>,
    pub old_declared_contracts: Vec<Felt>,
    pub declared_classes: Vec<DeclaredContract>,
    pub nonces: BTreeMap<ContractAddress, Nonce>,
    pub replaced_classes: Vec<DeployedContract>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct StorageDiff {
    pub key: StorageKey,
    pub value: StorageValue,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct DeployedContract {
    pub address: ContractAddress,
    pub class_hash: Felt,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct DeclaredContract {
    pub class_hash: ClassHash,
    pub compiled_class_hash: CompiledClassHash,
}

/// The state update type returns by `/get_state_update` endpoint, with `includeBlock=true`.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct StateUpdateWithBlock {
    pub state_update: StateUpdate,
    pub block: Block,
}

// The reason why we're not using the GasPrices from the `katana_primitives` crate is because
// the serde impl is different. So for now, lets just use starknet-rs types. The type isn't
// that complex anyway so the conversion is simple. But if we can use the primitive types, we
// should.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Block {
    #[serde(default)]
    pub block_hash: Option<BlockHash>,
    #[serde(default)]
    pub block_number: Option<BlockNumber>,
    pub parent_block_hash: BlockHash,
    pub timestamp: u64,
    pub sequencer_address: Option<ContractAddress>,
    #[serde(default)]
    pub state_root: Option<Felt>,
    #[serde(default)]
    pub transaction_commitment: Option<Felt>,
    #[serde(default)]
    pub event_commitment: Option<Felt>,
    pub status: BlockStatus,
    pub l1_da_mode: L1DataAvailabilityMode,
    pub l1_gas_price: ResourcePrice,
    pub l1_data_gas_price: ResourcePrice,
    pub transactions: Vec<ConfirmedTransaction>,
    pub transaction_receipts: Vec<ConfirmedReceipt>,
    #[serde(default)]
    pub starknet_version: Option<ProtocolVersion>,
}

// -- Conversion to Katana primitive types.

impl TryFrom<ContractClass> for katana_primitives::class::ContractClass {
    type Error = ConversionError;

    fn try_from(value: ContractClass) -> Result<Self, Self::Error> {
        match value {
            ContractClass::Legacy(class) => Ok(Self::Legacy(class)),
            ContractClass::Class(class) => {
                let class = SierraContractClass::try_from(class)?;
                Ok(Self::Class(class))
            }
        }
    }
}

impl From<StateDiff> for katana_primitives::state::StateUpdates {
    fn from(value: StateDiff) -> Self {
        let storage_updates = value
            .storage_diffs
            .into_iter()
            .map(|(addr, diffs)| {
                let storage_map = diffs.into_iter().map(|diff| (diff.key, diff.value)).collect();
                (addr, storage_map)
            })
            .collect();

        let deployed_contracts = value
            .deployed_contracts
            .into_iter()
            .map(|contract| (contract.address, contract.class_hash))
            .collect();

        let declared_classes = value
            .declared_classes
            .into_iter()
            .map(|contract| (contract.class_hash, contract.compiled_class_hash))
            .collect();

        let replaced_classes = value
            .replaced_classes
            .into_iter()
            .map(|contract| (contract.address, contract.class_hash))
            .collect();

        Self {
            storage_updates,
            declared_classes,
            replaced_classes,
            deployed_contracts,
            nonce_updates: value.nonces,
            deprecated_declared_classes: BTreeSet::from_iter(value.old_declared_contracts),
        }
    }
}
