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
use sir::Felt252;

use super::utils;
use crate::abstraction::StateProviderDb;

/// A helper trait to enforce that a type must implement both [StateProvider] and [StateReader].
pub(super) trait StateDb: StateProvider + StateReader {}
impl<T> StateDb for T where T: StateProvider + StateReader {}

impl<'a> StateReader for StateProviderDb<'a> {
    fn get_class_hash_at(&self, contract_address: &Address) -> Result<ClassHash, StateError> {
        match self.0.class_hash_of_contract(utils::to_address(contract_address)) {
            Ok(Some(value)) => Ok(utils::to_sir_class_hash(&value)),

            Ok(None) => Ok(ClassHash::default()),
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

            Ok(None) => Ok(Felt252::ZERO),
            Err(e) => Err(StateError::CustomError(e.to_string())),
        }
    }

    fn get_storage_at(&self, storage_entry: &StorageEntry) -> Result<sir::Felt252, StateError> {
        let address = utils::to_address(&storage_entry.0);
        let key = FieldElement::from_bytes_be(&storage_entry.1).unwrap();

        match self.0.storage(address, key) {
            Ok(Some(value)) => Ok(utils::to_sir_felt(&value)),

            Ok(None) => Ok(Felt252::ZERO),
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
        let state = self.0.read();
        if let Some((class, _)) = state.declared_classes.get(&hash) {
            Ok(Some(class.clone()))
        } else {
            state.inner.state_reader.class(hash)
        }
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

        let res = state
            .inner
            .get_class_hash_at(&utils::to_sir_address(&address))
            .map(|v| utils::to_class_hash(&v));

        match res {
            Ok(value) if value != FieldElement::ZERO => Ok(Some(value)),
            // check the inner state provider if the class hash is not found in the
            // cache state or if the returned class hash is zero
            Ok(_) | Err(StateError::NoneClassHash(_)) => {
                state.inner.state_reader.class_hash_of_contract(address)
            }
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }

    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        // check if the contract is deployed
        if self.class_hash_of_contract(address)?.is_none() {
            return Ok(None);
        }

        let state = self.0.read();
        let address = utils::to_sir_address(&address);

        match state.inner.get_nonce_at(&address) {
            Ok(value) => Ok(Some(utils::to_felt(&value))),

            Err(StateError::NoneNonce(_)) => Ok(None),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }

    // This function will ONLY return `None` if the contract is not deployed
    // and NOT if the contract is deployed but the storage is empty. Retrieving
    // non-existant storage will return FieldElement::ZERO due to the nature of
    // the StateReader trait.
    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        // check if the contract is deployed
        if self.class_hash_of_contract(address)?.is_none() {
            return Ok(None);
        }

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

#[cfg(test)]
mod tests {

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
    use sir::state::contract_class_cache::PermanentContractClassCache;
    use sir::state::state_api::{State, StateReader};
    use sir::transaction::ClassHash;
    use sir::Felt252;
    use starknet::macros::felt;

    use super::CachedState;
    use crate::implementation::sir::utils::{
        to_sir_address, to_sir_class_hash, to_sir_compiled_class, to_sir_felt,
    };
    use crate::StateProviderDb;

    fn new_sierra_class() -> (FlattenedSierraClass, CompiledClass) {
        let json = include_str!("../../../../contracts/compiled/cairo1_contract.json");
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
        let classes_cache = PermanentContractClassCache::default();
        let cached_state = CachedState::new(StateProviderDb(state), classes_cache);

        let address = to_sir_address(&ContractAddress::from(felt!("0x67")));
        let legacy_class_hash = to_sir_class_hash(&felt!("0x111"));

        let actual_class_hash = cached_state.get_class_hash_at(&address)?;
        let actual_nonce = cached_state.get_nonce_at(&address)?;
        let actual_storage_value =
            cached_state.get_storage_at(&(address.clone(), felt!("0x1").to_bytes_be()))?;
        let actual_compiled_hash = cached_state.get_compiled_class_hash(&actual_class_hash)?;
        let actual_class = cached_state.get_contract_class(&actual_class_hash)?;
        let actual_legacy_class = cached_state.get_contract_class(&legacy_class_hash)?;

        assert_eq!(actual_nonce, to_sir_felt(&felt!("0x7")));
        assert_eq!(actual_storage_value, to_sir_felt(&felt!("0x2")));
        assert_eq!(actual_class_hash, to_sir_class_hash(&felt!("0x123")));
        assert_eq!(actual_compiled_hash, to_sir_class_hash(&felt!("0x456")));
        assert_eq!(actual_class, to_sir_compiled_class(DEFAULT_OZ_ACCOUNT_CONTRACT_CASM.clone()));
        assert_eq!(
            actual_legacy_class,
            to_sir_compiled_class(DEFAULT_LEGACY_ERC20_CONTRACT_CASM.clone())
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
        let new_class_hash = felt!("0x777");
        let (new_sierra_class, new_compiled_sierra_class) = new_sierra_class();
        let new_compiled_hash = felt!("0xdead");
        // let new_legacy_compiled_hash = felt!("0x5678");

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
            sp.compiled_class_hash_of_class_hash(new_class_hash)?;

        assert_eq!(actual_new_nonce, None, "data shouldn't exist");
        assert_eq!(actual_new_class_hash, None, "data shouldn't exist");
        assert_eq!(actual_new_storage_value, None, "data shouldn't exist");
        assert_eq!(actual_new_legacy_class, None, "data should'nt exist");
        assert_eq!(actual_new_legacy_sierra_class, None, "data shouldn't exist");
        assert_eq!(actual_new_sierra_class, None, "data shouldn't exist");
        assert_eq!(actual_new_class, None, "data shouldn't exist");
        assert_eq!(actual_new_compiled_class_hash, None, "data shouldn't exist");
        assert_eq!(actual_new_legacy_compiled_hash, None, "data shouldn't exist");

        let classes_cache = PermanentContractClassCache::default();
        let cached_state = CachedState::new(StateProviderDb(sp), classes_cache);

        // insert some data to the cached state
        {
            let sir_address = to_sir_address(&new_address);
            let sir_legacy_class_hash = to_sir_class_hash(&new_legacy_class_hash);
            // let sir_legacy_compiled_hash = to_sir_felt(&new_compiled_hash);
            let sir_storage_key = new_storage_key.to_bytes_be();
            let sir_storage_value = to_sir_felt(&new_storage_value);
            let sir_class_hash = to_sir_class_hash(&new_class_hash);
            let sir_compiled_hash = to_sir_felt(&new_compiled_hash);

            let lock = &mut cached_state.0.write();
            let sir_state = &mut lock.inner;

            sir_state.increment_nonce(&sir_address)?;
            sir_state.set_class_hash_at(sir_address.clone(), sir_legacy_class_hash)?;
            sir_state.set_storage_at(&(sir_address.clone(), sir_storage_key), sir_storage_value);
            sir_state.set_contract_class(
                &sir_class_hash,
                &to_sir_compiled_class(new_compiled_sierra_class.clone()),
            )?;
            sir_state.set_compiled_class_hash(
                &Felt252::from_bytes_be(&sir_class_hash.0),
                &sir_compiled_hash,
            )?;
            sir_state.set_contract_class(
                &sir_legacy_class_hash,
                &to_sir_compiled_class(DEFAULT_LEGACY_UDC_CASM.clone()),
            )?;

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
        // let actual_new_legacy_compiled_hash =
        //     sp.compiled_class_hash_of_class_hash(new_legacy_class_hash)?;

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

        let classes_cache = PermanentContractClassCache::default();
        let cached_state = CachedState::new(StateProviderDb(sp), classes_cache);

        let actual_nonce = cached_state
            .get_nonce_at(&to_sir_address(&address))
            .expect("should return default value");
        let actual_storage_value = cached_state
            .get_storage_at(&(to_sir_address(&address), storage_key.to_bytes_be()))
            .expect("should return default value");
        let actual_class_hash = cached_state
            .get_class_hash_at(&to_sir_address(&address))
            .expect("should return default value");
        let actual_compiled_hash =
            cached_state.get_compiled_class_hash(&to_sir_class_hash(&class_hash));
        let actual_compiled_class =
            cached_state.get_contract_class(&to_sir_class_hash(&class_hash));
        let actual_edge_storage_value = cached_state
            .get_storage_at(&(to_sir_address(&edge_address), storage_key.to_bytes_be()))
            .expect("should return default value");

        assert_eq!(
            actual_nonce,
            Felt252::ZERO,
            "nonce of nonexistant contract should default to zero"
        );
        assert_eq!(
            actual_storage_value,
            Felt252::ZERO,
            "value of nonexistant contract and storage key should default to zero"
        );
        assert_eq!(
            actual_edge_storage_value,
            Felt252::ZERO,
            "value of nonexistant storage key but existant contract should default to zero"
        );
        assert_eq!(
            actual_class_hash,
            ClassHash::default(),
            "class hash of nonexistant contract should default to zero"
        );
        assert!(actual_compiled_hash
            .unwrap_err()
            .to_string()
            .contains("No compiled class hash found"));
        assert!(actual_compiled_class.unwrap_err().to_string().contains("No compiled class found"));

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
