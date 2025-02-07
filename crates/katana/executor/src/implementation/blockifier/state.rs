use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use blockifier::execution::contract_class::ContractClass as BlockifierContractClass;
use blockifier::state::cached_state;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{StateReader, StateResult};
use katana_cairo::starknet_api::core::{ClassHash, CompiledClassHash, Nonce};
use katana_cairo::starknet_api::state::StorageKey;
use katana_primitives::class::{self, ContractClass};
use katana_primitives::Felt;
use katana_provider::error::ProviderError;
use katana_provider::traits::contract::{ContractClassProvider, ContractClassProviderExt};
use katana_provider::traits::state::{StateProofProvider, StateProvider, StateRootProvider};
use katana_provider::ProviderResult;
use parking_lot::Mutex;
use tracing::trace;

use super::utils::{self};

#[derive(Debug, Clone)]
pub struct CachedState<'a> {
    pub(crate) inner: Arc<Mutex<CachedStateInner<'a>>>,
}

#[derive(Debug)]
pub(crate) struct CachedStateInner<'a> {
    pub(super) cached_state: cached_state::CachedState<StateProviderDb<'a>>,
    pub(super) declared_classes: HashMap<class::ClassHash, ContractClass>,
}

impl<'a> CachedState<'a> {
    pub(super) fn new(
        state: impl StateProvider + 'a,
        compiled_class_cache: Arc<Mutex<HashMap<class::ClassHash, BlockifierContractClass>>>,
    ) -> Self {
        let state = StateProviderDb::new(Box::new(state), compiled_class_cache);
        let cached_state = cached_state::CachedState::new(state);

        let declared_classes = HashMap::new();
        let inner = CachedStateInner { cached_state, declared_classes };

        Self { inner: Arc::new(Mutex::new(inner)) }
    }
}

impl<'a> ContractClassProvider for CachedState<'a> {
    fn class(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<ContractClass>> {
        let state = self.inner.lock();
        if let Some(class) = state.declared_classes.get(&hash) {
            Ok(Some(class.clone()))
        } else {
            state.cached_state.state.class(hash)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<katana_primitives::class::CompiledClassHash>> {
        let Ok(hash) = self.inner.lock().cached_state.get_compiled_class_hash(ClassHash(hash))
        else {
            return Ok(None);
        };

        if hash.0 == Felt::ZERO {
            Ok(None)
        } else {
            Ok(Some(hash.0))
        }
    }
}

impl<'a> StateProvider for CachedState<'a> {
    fn class_hash_of_contract(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::class::ClassHash>> {
        let Ok(hash) =
            self.inner.lock().cached_state.get_class_hash_at(utils::to_blk_address(address))
        else {
            return Ok(None);
        };

        if hash.0 == Felt::ZERO {
            Ok(None)
        } else {
            Ok(Some(hash.0))
        }
    }

    fn nonce(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::contract::Nonce>> {
        // check if the contract is deployed
        if self.class_hash_of_contract(address)?.is_none() {
            return Ok(None);
        }

        match self.inner.lock().cached_state.get_nonce_at(utils::to_blk_address(address)) {
            Ok(nonce) => Ok(Some(nonce.0)),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }

    fn storage(
        &self,
        address: katana_primitives::contract::ContractAddress,
        storage_key: katana_primitives::contract::StorageKey,
    ) -> ProviderResult<Option<katana_primitives::contract::StorageValue>> {
        // check if the contract is deployed
        if self.class_hash_of_contract(address)?.is_none() {
            return Ok(None);
        }

        let address = utils::to_blk_address(address);
        let key = StorageKey(storage_key.try_into().unwrap());

        match self.inner.lock().cached_state.get_storage_at(address, key) {
            Ok(value) => Ok(Some(value)),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }
}

impl<'a> StateProofProvider for CachedState<'a> {}
impl<'a> StateRootProvider for CachedState<'a> {}

#[derive(Debug)]
pub struct StateProviderDb<'a> {
    provider: Box<dyn StateProvider + 'a>,
    compiled_class_cache: Arc<Mutex<HashMap<class::ClassHash, BlockifierContractClass>>>,
}

impl<'a> Deref for StateProviderDb<'a> {
    type Target = Box<dyn StateProvider + 'a>;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}

impl<'a> StateProviderDb<'a> {
    pub fn new(
        provider: Box<dyn StateProvider + 'a>,
        compiled_class_cache: Arc<Mutex<HashMap<class::ClassHash, BlockifierContractClass>>>,
    ) -> Self {
        Self { provider, compiled_class_cache }
    }
}

impl<'a> StateReader for StateProviderDb<'a> {
    fn get_class_hash_at(
        &self,
        contract_address: katana_cairo::starknet_api::core::ContractAddress,
    ) -> StateResult<katana_cairo::starknet_api::core::ClassHash> {
        self.provider
            .class_hash_of_contract(utils::to_address(contract_address))
            .map(|v| ClassHash(v.unwrap_or_default()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_compiled_class_hash(
        &self,
        class_hash: katana_cairo::starknet_api::core::ClassHash,
    ) -> StateResult<katana_cairo::starknet_api::core::CompiledClassHash> {
        if let Some(hash) = self
            .provider
            .compiled_class_hash_of_class_hash(class_hash.0)
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            Ok(CompiledClassHash(hash))
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }

    fn get_compiled_contract_class(
        &self,
        class_hash: ClassHash,
    ) -> StateResult<BlockifierContractClass> {
        let mut class_cache = self.compiled_class_cache.lock();

        if let Some(class) = class_cache.get(&class_hash.0) {
            trace!(target: "executor", class = format!("{}", class_hash.to_hex_string()), "Class cache hit");
            return Ok(class.clone());
        }

        if let Some(class) = self
            .provider
            .compiled_class(class_hash.0)
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            trace!(target: "executor", class = format!("{}", class_hash.to_hex_string()), "Class cache miss");

            let class =
                utils::to_class(class).map_err(|e| StateError::StateReadError(e.to_string()))?;

            class_cache.insert(class_hash.0, class.clone());
            return Ok(class);
        }

        Err(StateError::UndeclaredClassHash(class_hash))
    }

    fn get_nonce_at(
        &self,
        contract_address: katana_cairo::starknet_api::core::ContractAddress,
    ) -> StateResult<katana_cairo::starknet_api::core::Nonce> {
        self.provider
            .nonce(utils::to_address(contract_address))
            .map(|n| Nonce(n.unwrap_or_default()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_storage_at(
        &self,
        contract_address: katana_cairo::starknet_api::core::ContractAddress,
        key: katana_cairo::starknet_api::state::StorageKey,
    ) -> StateResult<katana_cairo::starknet_api::hash::StarkHash> {
        self.storage(utils::to_address(contract_address), *key.0.key())
            .map(|v| v.unwrap_or_default())
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }
}
