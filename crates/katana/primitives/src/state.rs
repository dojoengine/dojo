use std::collections::HashMap;

use crate::contract::{
    ClassHash, CompiledClass, CompiledClassHash, ContractAddress, FlattenedSierraClass, Nonce,
    StorageKey, StorageValue,
};

/// State updates.
///
/// Represents all the state updates after performing some executions on a state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateUpdates {
    /// A mapping of contract addresses to their updated nonces.
    pub nonce_updates: HashMap<ContractAddress, Nonce>,
    /// A mapping of contract addresses to their updated storage entries.
    pub storage_updates: HashMap<ContractAddress, HashMap<StorageKey, StorageValue>>,
    /// A mapping of contract addresses to their updated class hashes.
    pub contract_updates: HashMap<ContractAddress, ClassHash>,
    /// A mapping of newly declared class hashes to their compiled class hashes.
    pub declared_classes: HashMap<ClassHash, CompiledClassHash>,
}

/// State update with declared classes definition.
#[derive(Debug, Default, Clone)]
pub struct StateUpdatesWithDeclaredClasses {
    /// State updates.
    pub state_updates: StateUpdates,
    /// A mapping of class hashes to their sierra classes definition.
    pub declared_sierra_classes: HashMap<ClassHash, FlattenedSierraClass>,
    /// A mapping of class hashes to their compiled classes definition.
    pub declared_compiled_classes: HashMap<ClassHash, CompiledClass>,
}
