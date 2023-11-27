use std::collections::HashMap;
use std::sync::Arc;

use blockifier::execution::contract_class::ContractClass;
use blockifier::state::cached_state::{CachedState, CommitmentStateDiff};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{State, StateReader, StateResult};
use katana_primitives::contract::SierraClass;
use katana_provider::traits::state::StateProvider;
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;
use tokio::sync::RwLock as AsyncRwLock;

pub struct StateRefDb(Box<dyn StateProvider>);

impl StateRefDb {
    pub fn new(provider: impl StateProvider + 'static) -> Self {
        Self(Box::new(provider))
    }
}

impl<T> From<T> for StateRefDb
where
    T: StateProvider + 'static,
{
    fn from(provider: T) -> Self {
        Self::new(provider)
    }
}

impl StateReader for StateRefDb {
    fn get_nonce_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> blockifier::state::state_api::StateResult<Nonce> {
        StateProvider::nonce(&self.0, contract_address.into())
            .map(|n| Nonce(n.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_storage_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
        key: starknet_api::state::StorageKey,
    ) -> blockifier::state::state_api::StateResult<starknet_api::hash::StarkFelt> {
        StateProvider::storage(&self.0, contract_address.into(), (*key.0.key()).into())
            .map(|v| v.unwrap_or_default().into())
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_class_hash_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> blockifier::state::state_api::StateResult<starknet_api::core::ClassHash> {
        StateProvider::class_hash_of_contract(&self.0, contract_address.into())
            .map(|v| ClassHash(v.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: starknet_api::core::ClassHash,
    ) -> blockifier::state::state_api::StateResult<starknet_api::core::CompiledClassHash> {
        if let Some(hash) =
            StateProvider::compiled_class_hash_of_class_hash(&self.0, class_hash.0.into())
                .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            Ok(CompiledClassHash(hash.into()))
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &starknet_api::core::ClassHash,
    ) -> blockifier::state::state_api::StateResult<
        blockifier::execution::contract_class::ContractClass,
    > {
        if let Some(class) = StateProvider::class(&self.0, class_hash.0.into())
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            Ok(class)
        } else {
            Err(StateError::UndeclaredClassHash(*class_hash))
        }
    }
}

#[derive(Clone)]
pub struct CachedStateWrapper<S: StateReader> {
    inner: Arc<AsyncRwLock<CachedState<S>>>,
    sierra_class: Arc<AsyncRwLock<HashMap<katana_primitives::contract::ClassHash, SierraClass>>>,
}

impl<S: StateReader> CachedStateWrapper<S> {
    pub fn new(db: S) -> Self {
        Self {
            sierra_class: Default::default(),
            inner: Arc::new(AsyncRwLock::new(CachedState::new(db))),
        }
    }

    pub fn inner_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, CachedState<S>> {
        tokio::task::block_in_place(|| self.inner.blocking_write())
    }

    pub fn sierra_class(
        &self,
    ) -> tokio::sync::RwLockReadGuard<
        '_,
        HashMap<katana_primitives::contract::ClassHash, SierraClass>,
    > {
        tokio::task::block_in_place(|| self.sierra_class.blocking_read())
    }

    pub fn sierra_class_mut(
        &self,
    ) -> tokio::sync::RwLockWriteGuard<
        '_,
        HashMap<katana_primitives::contract::ClassHash, SierraClass>,
    > {
        tokio::task::block_in_place(|| self.sierra_class.blocking_write())
    }
}

impl<S: StateReader> State for CachedStateWrapper<S> {
    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        self.inner_mut().increment_nonce(contract_address)
    }

    fn set_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        self.inner_mut().set_class_hash_at(contract_address, class_hash)
    }

    fn set_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
        compiled_class_hash: CompiledClassHash,
    ) -> StateResult<()> {
        self.inner_mut().set_compiled_class_hash(class_hash, compiled_class_hash)
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        self.inner_mut().set_contract_class(class_hash, contract_class)
    }

    fn set_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
        value: StarkFelt,
    ) {
        self.inner_mut().set_storage_at(contract_address, key, value)
    }

    fn to_state_diff(&self) -> CommitmentStateDiff {
        self.inner_mut().to_state_diff()
    }
}

impl<Db: StateReader> StateReader for CachedStateWrapper<Db> {
    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        self.inner_mut().get_class_hash_at(contract_address)
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.inner_mut().get_compiled_class_hash(class_hash)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.inner_mut().get_compiled_contract_class(class_hash)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        self.inner_mut().get_nonce_at(contract_address)
    }

    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        self.inner_mut().get_storage_at(contract_address, key)
    }
}
