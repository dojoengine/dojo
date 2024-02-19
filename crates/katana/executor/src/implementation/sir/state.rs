use std::collections::HashMap;
use std::sync::Arc;

use katana_primitives::class::{CompiledClass, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::FieldElement;
use katana_provider::error::ProviderError;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use katana_provider::ProviderResult;
use parking_lot::RwLock;
use sir::core::errors::state_errors::StateError;
use sir::state::cached_state;
use sir::state::contract_class_cache::ContractClassCache;
use sir::state::state_api::StateReader;
use sir::state::state_cache::StorageEntry;
use sir::transaction::{Address, ClassHash, CompiledClassHash};

use super::utils;
use crate::abstraction::StateProviderDb;

/// A helper trait to enforce that a type must implement both [StateProvider] and [StateReader].
pub(super) trait StateDb: StateProvider + StateReader {}
impl<T> StateDb for T where T: StateProvider + StateReader {}

impl<'a> StateReader for StateProviderDb<'a> {
    fn get_class_hash_at(&self, contract_address: &Address) -> Result<ClassHash, StateError> {
        match self.0.class_hash_of_contract(utils::to_address(contract_address)) {
            Ok(Some(value)) => Ok(utils::to_sir_class_hash(&value)),

            Ok(None) => Err(StateError::NoneClassHash(contract_address.clone())),
            Err(e) => Err(StateError::CustomError(e.to_string())),
        }
    }

    fn get_compiled_class_hash(
        &self,
        class_hash: &ClassHash,
    ) -> Result<CompiledClassHash, StateError> {
        match self.0.compiled_class_hash_of_class_hash(utils::to_class_hash(class_hash)) {
            Ok(Some(value)) => Ok(utils::to_sir_class_hash(&value)),

            Ok(None) => Err(StateError::NoneCompiledHash(*class_hash)),
            Err(e) => Err(StateError::CustomError(e.to_string())),
        }
    }

    fn get_contract_class(
        &self,
        class_hash: &ClassHash,
    ) -> Result<sir::services::api::contract_classes::compiled_class::CompiledClass, StateError>
    {
        match self.0.class(utils::to_class_hash(class_hash)) {
            Ok(Some(value)) => Ok(utils::to_sir_compiled_class(value)),

            Ok(None) => Err(StateError::NoneCompiledClass(*class_hash)),
            Err(e) => Err(StateError::CustomError(e.to_string())),
        }
    }

    fn get_nonce_at(&self, contract_address: &Address) -> Result<sir::Felt252, StateError> {
        match self.0.nonce(utils::to_address(contract_address)) {
            Ok(Some(value)) => Ok(utils::to_sir_felt(&value)),

            Ok(None) => Err(StateError::NoneNonce(contract_address.clone())),
            Err(e) => Err(StateError::CustomError(e.to_string())),
        }
    }

    fn get_storage_at(&self, storage_entry: &StorageEntry) -> Result<sir::Felt252, StateError> {
        let address = utils::to_address(&storage_entry.0);
        let key = FieldElement::from_bytes_be(&storage_entry.1).unwrap();

        match self.0.storage(address, key) {
            Ok(Some(value)) => Ok(utils::to_sir_felt(&value)),

            Ok(None) => Err(StateError::NoneStorage(storage_entry.clone())),
            Err(e) => Err(StateError::CustomError(e.to_string())),
        }
    }
}

type DeclaredClass = (CompiledClass, Option<FlattenedSierraClass>);

#[derive(Debug, Default)]
pub(super) struct CachedState<S, C>(pub(super) Arc<RwLock<CachedStateInner<S, C>>>)
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync;

impl<S, C> Clone for CachedState<S, C>
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync,
{
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

#[derive(Debug, Default)]
pub(super) struct CachedStateInner<S, C>
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync,
{
    pub(super) inner: cached_state::CachedState<S, C>,
    pub(super) declared_classes: HashMap<katana_primitives::class::ClassHash, DeclaredClass>,
}

impl<S, C> CachedState<S, C>
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync,
{
    pub(super) fn new(state: S, classes_cache: C) -> Self {
        let declared_classes = HashMap::new();
        let cached_state = cached_state::CachedState::new(Arc::new(state), Arc::new(classes_cache));
        let inner = CachedStateInner { inner: cached_state, declared_classes };
        Self(Arc::new(RwLock::new(inner)))
    }
}

impl<S, C> ContractClassProvider for CachedState<S, C>
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync,
{
    fn class(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<CompiledClass>> {
        let inner = self.0.read();
        let class = inner.declared_classes.get(&hash).map(|(class, _)| class.clone());
        Ok(class)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<katana_primitives::class::CompiledClassHash>> {
        let state = self.0.read();
        let hash = utils::to_sir_class_hash(&hash);

        match state.inner.get_compiled_class_hash(&hash) {
            Ok(value) => Ok(Some(utils::to_class_hash(&value))),

            Err(StateError::NoneCompiledHash(_)) => Ok(None),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }

    fn sierra_class(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<FlattenedSierraClass>> {
        let state = self.0.read();
        if let Some((_, sierra)) = state.declared_classes.get(&hash) {
            Ok(sierra.clone())
        } else {
            state.inner.state_reader.sierra_class(hash)
        }
    }
}

impl<S, C> StateProvider for CachedState<S, C>
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync,
{
    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::class::ClassHash>> {
        let state = self.0.read();
        let address = utils::to_sir_address(&address);

        match state.inner.get_class_hash_at(&address) {
            Ok(value) => Ok(Some(utils::to_class_hash(&value))),

            Err(StateError::NoneClassHash(_)) => Ok(None),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }

    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let state = self.0.read();
        let address = utils::to_sir_address(&address);

        match state.inner.get_nonce_at(&address) {
            Ok(value) => Ok(Some(utils::to_felt(&value))),

            Err(StateError::NoneNonce(_)) => Ok(None),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let state = self.0.read();

        let address = utils::to_sir_address(&address);
        let key = utils::to_sir_felt(&storage_key);

        match state.inner.get_storage_at(&(address, key.to_bytes_be())) {
            Ok(value) => Ok(Some(utils::to_felt(&value))),

            Err(StateError::NoneStorage(_)) => Ok(None),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }
}

impl<S, C> StateReader for CachedState<S, C>
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync,
{
    fn get_class_hash_at(&self, contract_address: &Address) -> Result<ClassHash, StateError> {
        self.0.read().inner.get_class_hash_at(contract_address)
    }

    fn get_compiled_class_hash(
        &self,
        class_hash: &ClassHash,
    ) -> Result<CompiledClassHash, StateError> {
        self.0.read().inner.get_compiled_class_hash(class_hash)
    }

    fn get_contract_class(
        &self,
        class_hash: &ClassHash,
    ) -> Result<sir::services::api::contract_classes::compiled_class::CompiledClass, StateError>
    {
        self.0.read().inner.get_contract_class(class_hash)
    }

    fn get_nonce_at(&self, contract_address: &Address) -> Result<sir::Felt252, StateError> {
        self.0.read().inner.get_nonce_at(contract_address)
    }

    fn get_storage_at(&self, storage_entry: &StorageEntry) -> Result<sir::Felt252, StateError> {
        self.0.read().inner.get_storage_at(storage_entry)
    }
}
