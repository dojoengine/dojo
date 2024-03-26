use std::collections::HashMap;
use std::sync::Arc;

use blockifier::state::cached_state::{self, GlobalContractCache};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{StateReader, StateResult};
use katana_primitives::class::{CompiledClass, FlattenedSierraClass};
use katana_primitives::FieldElement;
use katana_provider::error::ProviderError;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use katana_provider::ProviderResult;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use starknet_api::core::{ClassHash, CompiledClassHash, Nonce, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;

use super::utils::{self};
use crate::StateProviderDb;

/// A helper trait to enforce that a type must implement both [StateProvider] and [StateReader].
pub(super) trait StateDb: StateProvider + StateReader {}

impl<T> StateDb for T where T: StateProvider + StateReader {}

impl<'a> StateReader for StateProviderDb<'a> {
    fn get_class_hash_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<starknet_api::core::ClassHash> {
        self.0
            .class_hash_of_contract(utils::to_address(contract_address))
            .map(|v| ClassHash(v.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: starknet_api::core::ClassHash,
    ) -> StateResult<starknet_api::core::CompiledClassHash> {
        if let Some(hash) = self
            .0
            .compiled_class_hash_of_class_hash(class_hash.0.into())
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            Ok(CompiledClassHash(hash.into()))
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<blockifier::execution::contract_class::ContractClass> {
        if let Some(class) = self
            .0
            .class(class_hash.0.into())
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            let class =
                utils::to_class(class).map_err(|e| StateError::StateReadError(e.to_string()))?;

            Ok(class)
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }

    fn get_nonce_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<starknet_api::core::Nonce> {
        self.0
            .nonce(utils::to_address(contract_address))
            .map(|n| Nonce(n.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_storage_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
        key: starknet_api::state::StorageKey,
    ) -> StateResult<starknet_api::hash::StarkFelt> {
        self.0
            .storage(utils::to_address(contract_address), (*key.0.key()).into())
            .map(|v| v.unwrap_or_default().into())
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }
}

pub(super) struct CachedState<S: StateDb>(pub(super) Arc<RwLock<CachedStateInner<S>>>);

impl<S: StateDb> Clone for CachedState<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

type DeclaredClass = (CompiledClass, Option<FlattenedSierraClass>);

#[derive(Debug)]
pub(super) struct CachedStateInner<S: StateReader> {
    pub(super) inner: cached_state::CachedState<S>,
    pub(super) declared_classes: HashMap<katana_primitives::class::ClassHash, DeclaredClass>,
}

impl<S: StateDb> CachedState<S> {
    pub(super) fn new(state: S) -> Self {
        let declared_classes = HashMap::new();
        let cached_state = cached_state::CachedState::new(state, GlobalContractCache::default());
        let inner = CachedStateInner { inner: cached_state, declared_classes };
        Self(Arc::new(RwLock::new(inner)))
    }

    pub(super) fn read(&self) -> RwLockReadGuard<'_, CachedStateInner<S>> {
        self.0.read()
    }

    pub(super) fn write(&self) -> RwLockWriteGuard<'_, CachedStateInner<S>> {
        self.0.write()
    }
}

impl<S: StateDb> ContractClassProvider for CachedState<S> {
    fn class(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<CompiledClass>> {
        let state = self.read();
        if let Some((class, _)) = state.declared_classes.get(&hash) {
            Ok(Some(class.clone()))
        } else {
            state.inner.state.class(hash)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<katana_primitives::class::CompiledClassHash>> {
        let Ok(hash) = self.write().inner.get_compiled_class_hash(ClassHash(hash.into())) else {
            return Ok(None);
        };
        Ok(Some(hash.0.into()))
    }

    fn sierra_class(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<FlattenedSierraClass>> {
        let state = self.read();
        if let Some((_, sierra)) = state.declared_classes.get(&hash) {
            Ok(sierra.clone())
        } else {
            state.inner.state.sierra_class(hash)
        }
    }
}

impl<S: StateDb> StateProvider for CachedState<S> {
    fn class_hash_of_contract(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::class::ClassHash>> {
        let Ok(hash) = self.write().inner.get_class_hash_at(utils::to_blk_address(address)) else {
            return Ok(None);
        };

        let hash = hash.0.into();
        if hash == FieldElement::ZERO { Ok(None) } else { Ok(Some(hash)) }
    }

    fn nonce(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::contract::Nonce>> {
        // check if the contract is deployed
        if self.class_hash_of_contract(address)?.is_none() {
            return Ok(None);
        }

        match self.0.write().inner.get_nonce_at(utils::to_blk_address(address)) {
            Ok(nonce) => Ok(Some(nonce.0.into())),
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
        let key = StorageKey(patricia_key!(storage_key));

        match self.write().inner.get_storage_at(address, key) {
            Ok(value) => Ok(Some(value.into())),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }
}

impl<S: StateDb> StateReader for CachedState<S> {
    fn get_class_hash_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<ClassHash> {
        self.write().inner.get_class_hash_at(contract_address)
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.write().inner.get_compiled_class_hash(class_hash)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<blockifier::execution::contract_class::ContractClass> {
        self.write().inner.get_compiled_contract_class(class_hash)
    }

    fn get_nonce_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<Nonce> {
        self.write().inner.get_nonce_at(contract_address)
    }

    fn get_storage_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
        key: StorageKey,
    ) -> StateResult<starknet_api::hash::StarkFelt> {
        self.write().inner.get_storage_at(contract_address, key)
    }
}

#[cfg(test)]
mod tests {

    use blockifier::state::state_api::{State, StateReader};
    use katana_primitives::class::{CompiledClass, FlattenedSierraClass};
    use katana_primitives::contract::ContractAddress;
    use katana_primitives::genesis::constant::{
        DEFAULT_LEGACY_ERC20_CONTRACT_CASM, DEFAULT_LEGACY_UDC_CASM, DEFAULT_OZ_ACCOUNT_CONTRACT,
        DEFAULT_OZ_ACCOUNT_CONTRACT_CASM,
    };
    use katana_primitives::utils::class::{parse_compiled_class, parse_sierra_class};
    use katana_primitives::FieldElement;
    use katana_provider::providers::in_memory::InMemoryProvider;
    use katana_provider::traits::contract::ContractClassWriter;
    use katana_provider::traits::state::{StateFactoryProvider, StateProvider, StateWriter};
    use starknet::macros::felt;

    use super::{CachedState, *};
    use crate::StateProviderDb;

    fn new_sierra_class() -> (FlattenedSierraClass, CompiledClass) {
        let json = include_str!("../../../../primitives/contracts/compiled/cairo1_contract.json");
        let artifact = serde_json::from_str(json).unwrap();
        let compiled_class = parse_compiled_class(artifact).unwrap();
        let sierra_class = parse_sierra_class(json).unwrap().flatten().unwrap();
        (sierra_class, compiled_class)
    }

    fn state_provider() -> Box<dyn StateProvider> {
        let address = ContractAddress::from(felt!("0x67"));
        let nonce = felt!("0x7");
        let storage_key = felt!("0x1");
        let storage_value = felt!("0x2");
        let class_hash = felt!("0x123");
        let compiled_hash = felt!("0x456");
        let sierra_class = DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten().unwrap();
        let class = DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone();
        let legacy_class_hash = felt!("0x111");
        let legacy_class = DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone();

        let provider = InMemoryProvider::new();
        provider.set_nonce(address, nonce).unwrap();
        provider.set_class_hash_of_contract(address, class_hash).unwrap();
        provider.set_storage(address, storage_key, storage_value).unwrap();
        provider.set_compiled_class_hash_of_class_hash(class_hash, compiled_hash).unwrap();
        provider.set_class(class_hash, class).unwrap();
        provider.set_sierra_class(class_hash, sierra_class).unwrap();
        provider.set_class(legacy_class_hash, legacy_class).unwrap();

        provider.latest().unwrap()
    }

    #[test]
    fn can_fetch_from_inner_state_provider() -> anyhow::Result<()> {
        let state = state_provider();
        let mut cached_state = CachedState::new(StateProviderDb(state));

        let address = ContractAddress::from(felt!("0x67"));
        let legacy_class_hash = felt!("0x111");
        let storage_key = felt!("0x1");

        let api_address = utils::to_blk_address(address);
        let actual_class_hash = cached_state.get_class_hash_at(api_address)?;
        let actual_nonce = cached_state.get_nonce_at(api_address)?;
        let actual_storage_value =
            cached_state.get_storage_at(api_address, StorageKey(patricia_key!(storage_key)))?;
        let actual_compiled_hash = cached_state.get_compiled_class_hash(actual_class_hash)?;
        let actual_class = cached_state.get_compiled_contract_class(actual_class_hash)?;
        let actual_legacy_class =
            cached_state.get_compiled_contract_class(ClassHash(legacy_class_hash.into()))?;

        assert_eq!(actual_nonce.0, felt!("0x7").into());
        assert_eq!(actual_storage_value, felt!("0x2").into());
        assert_eq!(actual_class_hash.0, felt!("0x123").into());
        assert_eq!(actual_compiled_hash.0, felt!("0x456").into());
        assert_eq!(
            actual_class,
            utils::to_class(DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone()).unwrap()
        );
        assert_eq!(
            actual_legacy_class,
            utils::to_class(DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone()).unwrap()
        );

        Ok(())
    }

    #[test]
    fn can_fetch_as_state_provider() -> anyhow::Result<()> {
        let sp = state_provider();

        // cache_state native data
        let new_address = ContractAddress::from(felt!("0xdead"));
        let new_storage_key = felt!("0xf00");
        let new_storage_value = felt!("0xba");
        let new_legacy_class_hash = felt!("0x1234");
        let new_legacy_class = DEFAULT_LEGACY_UDC_CASM.clone();
        let new_legacy_compiled_hash = felt!("0x5678");
        let new_class_hash = felt!("0x777");
        let (new_sierra_class, new_compiled_sierra_class) = new_sierra_class();
        let new_compiled_hash = felt!("0xdead");

        // we're asserting that the underlying state provider doesnt have cache state native data

        let actual_new_nonce = sp.nonce(new_address)?;
        let actual_new_class_hash = sp.class_hash_of_contract(new_address)?;
        let actual_new_storage_value = sp.storage(new_address, new_storage_key)?;
        let actual_new_legacy_class = sp.class(new_legacy_class_hash)?;
        let actual_new_legacy_sierra_class = sp.class(new_legacy_class_hash)?;
        let actual_new_sierra_class = sp.sierra_class(new_class_hash)?;
        let actual_new_class = sp.class(new_class_hash)?;
        let actual_new_compiled_class_hash =
            sp.compiled_class_hash_of_class_hash(new_class_hash)?;
        let actual_new_legacy_compiled_hash =
            sp.compiled_class_hash_of_class_hash(new_legacy_class_hash)?;

        assert_eq!(actual_new_nonce, None, "data shouldn't exist");
        assert_eq!(actual_new_class_hash, None, "data shouldn't exist");
        assert_eq!(actual_new_storage_value, None, "data shouldn't exist");
        assert_eq!(actual_new_legacy_class, None, "data should'nt exist");
        assert_eq!(actual_new_legacy_sierra_class, None, "data shouldn't exist");
        assert_eq!(actual_new_sierra_class, None, "data shouldn't exist");
        assert_eq!(actual_new_class, None, "data shouldn't exist");
        assert_eq!(actual_new_compiled_class_hash, None, "data shouldn't exist");
        assert_eq!(actual_new_legacy_compiled_hash, None, "data shouldn't exist");

        let cached_state = CachedState::new(StateProviderDb(sp));

        // insert some data to the cached state
        {
            let lock = &mut cached_state.0.write();
            let blk_state = &mut lock.inner;

            let address = utils::to_blk_address(new_address);
            let storage_key = StorageKey(patricia_key!(new_storage_key));
            let storage_value = new_storage_value.into();
            let class_hash = ClassHash(new_class_hash.into());
            let class = utils::to_class(new_compiled_sierra_class.clone()).unwrap();
            let compiled_hash = CompiledClassHash(new_compiled_hash.into());
            let legacy_class_hash = ClassHash(new_legacy_class_hash.into());
            let legacy_class = utils::to_class(DEFAULT_LEGACY_UDC_CASM.clone()).unwrap();
            let legacy_compiled_hash = CompiledClassHash(new_legacy_compiled_hash.into());

            blk_state.increment_nonce(address)?;
            blk_state.set_class_hash_at(address, legacy_class_hash)?;
            blk_state.set_storage_at(address, storage_key, storage_value)?;
            blk_state.set_contract_class(class_hash, class)?;
            blk_state.set_compiled_class_hash(class_hash, compiled_hash)?;
            blk_state.set_contract_class(legacy_class_hash, legacy_class)?;
            blk_state.set_compiled_class_hash(legacy_class_hash, legacy_compiled_hash)?;

            let declared_classes = &mut lock.declared_classes;
            declared_classes.insert(new_legacy_class_hash, (new_legacy_class.clone(), None));
            declared_classes.insert(
                new_class_hash,
                (new_compiled_sierra_class.clone(), Some(new_sierra_class.clone())),
            );
        }

        // assert that can fetch data from the underlyign state provider
        let sp: Box<dyn StateProvider> = Box::new(cached_state);

        let address = ContractAddress::from(felt!("0x67"));
        let class_hash = felt!("0x123");
        let legacy_class_hash = felt!("0x111");

        let actual_class_hash = sp.class_hash_of_contract(address)?;
        let actual_nonce = sp.nonce(address)?;
        let actual_storage_value = sp.storage(address, felt!("0x1"))?;
        let actual_class = sp.class(class_hash)?;
        let actual_sierra_class = sp.sierra_class(class_hash)?;
        let actual_compiled_hash = sp.compiled_class_hash_of_class_hash(class_hash)?;
        let actual_legacy_class = sp.class(legacy_class_hash)?;

        assert_eq!(actual_nonce, Some(felt!("0x7")));
        assert_eq!(actual_class_hash, Some(class_hash));
        assert_eq!(actual_storage_value, Some(felt!("0x2")));
        assert_eq!(actual_compiled_hash, Some(felt!("0x456")));
        assert_eq!(actual_class, Some(DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone()));
        assert_eq!(actual_sierra_class, Some(DEFAULT_OZ_ACCOUNT_CONTRACT.clone().flatten()?));
        assert_eq!(actual_legacy_class, Some(DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone()));

        // assert that can fetch data native to the cached state from the state provider

        let actual_new_class_hash = sp.class_hash_of_contract(new_address)?;
        let actual_new_nonce = sp.nonce(new_address)?;
        let actual_new_storage_value = sp.storage(new_address, new_storage_key)?;
        let actual_new_class = sp.class(new_class_hash)?;
        let actual_new_sierra = sp.sierra_class(new_class_hash)?;
        let actual_new_compiled_hash = sp.compiled_class_hash_of_class_hash(new_class_hash)?;
        let actual_legacy_class = sp.class(new_legacy_class_hash)?;
        let actual_legacy_sierra = sp.sierra_class(new_legacy_class_hash)?;
        let actual_new_legacy_compiled_hash =
            sp.compiled_class_hash_of_class_hash(new_legacy_class_hash)?;

        assert_eq!(actual_new_nonce, Some(felt!("0x1")), "data should be in cached state");
        assert_eq!(
            actual_new_class_hash,
            Some(new_legacy_class_hash),
            "data should be in cached state"
        );
        assert_eq!(
            actual_new_storage_value,
            Some(new_storage_value),
            "data should be in cached state"
        );
        assert_eq!(actual_new_class, Some(new_compiled_sierra_class));
        assert_eq!(actual_new_sierra, Some(new_sierra_class));
        assert_eq!(actual_new_compiled_hash, Some(new_compiled_hash));
        assert_eq!(actual_legacy_class, Some(new_legacy_class));
        assert_eq!(actual_legacy_sierra, None, "legacy class should not have sierra class");
        assert_eq!(
            actual_new_legacy_compiled_hash,
            Some(new_legacy_compiled_hash),
            "data should
        be in cached state"
        );

        Ok(())
    }

    #[test]
    fn fetch_non_existant_data() -> anyhow::Result<()> {
        let db = InMemoryProvider::new();

        let address = ContractAddress::from(felt!("0x1"));
        let class_hash = felt!("0x123");
        let storage_key = felt!("0x1");

        // edge case: the StateProvider::storage impl of CachedState will return
        // default value for non-existant storage key of an existant contract. It will
        // only return None if the contract does not exist. The intended behaviour for
        // StateProvider::storage is to return None if the storage key or contract address
        // does not exist.
        let edge_address = ContractAddress::from(felt!("0x2"));
        db.set_class_hash_of_contract(edge_address, class_hash)?;

        let sp = db.latest()?;

        let mut cached_state = CachedState::new(StateProviderDb(sp));

        let api_address = utils::to_blk_address(address);
        let api_storage_key = StorageKey(patricia_key!(storage_key));
        let api_class_hash = ClassHash(class_hash.into());

        let actual_nonce =
            cached_state.get_nonce_at(api_address).expect("should return default value");
        let actual_storage_value = cached_state
            .get_storage_at(api_address, api_storage_key)
            .expect("should return default value");
        let actual_class_hash =
            cached_state.get_class_hash_at(api_address).expect("should return default value");
        let actual_compiled_hash = cached_state.get_compiled_class_hash(api_class_hash);
        let actual_compiled_class = cached_state.get_compiled_contract_class(api_class_hash);
        let actual_edge_storage_value = cached_state
            .get_storage_at(utils::to_blk_address(edge_address), api_storage_key)
            .expect("should return default value");

        assert_eq!(
            actual_nonce,
            Default::default(),
            "nonce of nonexistant contract should default to zero"
        );
        assert_eq!(
            actual_storage_value,
            Default::default(),
            "value of nonexistant contract and storage key should default to zero"
        );
        assert_eq!(
            actual_edge_storage_value,
            Default::default(),
            "value of nonexistant storage key but existant contract should default to zero"
        );
        assert_eq!(
            actual_class_hash,
            ClassHash::default(),
            "class hash of nonexistant contract should default to zero"
        );
        assert!(actual_compiled_hash.unwrap_err().to_string().contains("is not declared"));
        assert!(actual_compiled_class.unwrap_err().to_string().contains("is not declared"));

        let sp: Box<dyn StateProvider> = Box::new(cached_state);

        let actual_nonce = sp.nonce(address)?;
        let actual_storage_value = sp.storage(address, storage_key)?;
        let actual_edge_storage_value = sp.storage(edge_address, storage_key)?;
        let actual_class_hash = sp.class_hash_of_contract(address)?;
        let actual_compiled_hash = sp.compiled_class_hash_of_class_hash(class_hash)?;
        let actual_class = sp.class(class_hash)?;

        assert_eq!(actual_nonce, None, "nonce of nonexistant contract should be None");
        assert_eq!(actual_class_hash, None, "class hash of nonexistant contract should be None");
        assert_eq!(actual_storage_value, None, "value of nonexistant contract should be None");
        assert_eq!(
            actual_edge_storage_value,
            Some(FieldElement::ZERO),
            "edge case: value of nonexistant storage key but existant contract should return zero"
        );
        assert_eq!(actual_compiled_hash, None);
        assert_eq!(actual_class, None);

        Ok(())
    }
}
