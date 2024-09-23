use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use crate::contract::{ContractAddress, Nonce, StorageKey, StorageValue};

/// State updates.
///
/// Represents all the state updates after performing some executions on a state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateUpdates {
    /// A mapping of contract addresses to their updated nonces.
    pub nonce_updates: BTreeMap<ContractAddress, Nonce>,
    /// A mapping of contract addresses to their updated storage entries.
    pub storage_updates: BTreeMap<ContractAddress, BTreeMap<StorageKey, StorageValue>>,
    /// A mapping of contract addresses to their updated class hashes.
    pub deployed_contracts: BTreeMap<ContractAddress, ClassHash>,
    /// A mapping of newly declared class hashes to their compiled class hashes.
    pub declared_classes: BTreeMap<ClassHash, CompiledClassHash>,
    /// A mapping of newly declared legacy class hashes.
    pub deprecated_declared_classes: BTreeSet<ClassHash>,
    /// A mapping of replaced contract addresses to their new class hashes ie using `replace_class` syscall.
    pub replaced_classes: BTreeMap<ContractAddress, ClassHash>,
}

/// State update with declared classes definition.
#[derive(Debug, Default, Clone)]
pub struct StateUpdatesWithDeclaredClasses {
    /// State updates.
    pub state_updates: StateUpdates,
    /// A mapping of class hashes to their sierra classes definition.
    pub declared_sierra_classes: BTreeMap<ClassHash, FlattenedSierraClass>,
    /// A mapping of class hashes to their compiled classes definition.
    pub declared_compiled_classes: BTreeMap<ClassHash, CompiledClass>,
}
