use std::collections::HashMap;
use std::sync::Arc;

use blockifier::execution::contract_class::ContractClass;
use blockifier::state::cached_state::{CachedState, CommitmentStateDiff};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{State, StateReader, StateResult};
use starknet::core::types::FlattenedSierraClass;
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::trace;

use super::{AsStateRefDb, StateExt, StateExtRef, StateRefDb};

#[derive(Clone, Debug, Default)]
pub struct StorageRecord {
    pub nonce: Nonce,
    pub storage: HashMap<StorageKey, StarkFelt>,
}

#[derive(Clone, Debug)]
pub struct ClassRecord {
    /// The compiled contract class.
    pub class: ContractClass,
    /// The hash of a compiled Sierra class (if the class is a Sierra class, otherwise
    /// for legacy contract, it is the same as the class hash).
    pub compiled_hash: CompiledClassHash,
}

/// A cached state database which fallbacks to an inner database if the data
/// is not found in the cache.
///
/// The data that has been fetched from the inner database is cached in the
/// cache database.
#[derive(Clone, Debug)]
pub struct CachedDb<Db: StateExtRef> {
    /// A map of class hash to its class definition.
    pub classes: HashMap<ClassHash, ClassRecord>,
    /// A map of contract address to its class hash.
    pub contracts: HashMap<ContractAddress, ClassHash>,
    /// A map of contract address to the contract information.
    pub storage: HashMap<ContractAddress, StorageRecord>,
    /// A map of class hash to its Sierra class definition (if any).
    pub sierra_classes: HashMap<ClassHash, FlattenedSierraClass>,
    /// Inner database to fallback to when the data is not found in the cache.
    pub db: Db,
}

impl<Db> CachedDb<Db>
where
    Db: StateExtRef,
{
    /// Construct a new [CachedDb] with an inner database.
    pub fn new(db: Db) -> Self {
        Self {
            db,
            classes: HashMap::new(),
            storage: HashMap::new(),
            contracts: HashMap::new(),
            sierra_classes: HashMap::new(),
        }
    }
}

impl<Db> State for CachedDb<Db>
where
    Db: StateExtRef,
{
    fn set_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
        value: StarkFelt,
    ) {
        self.storage.entry(contract_address).or_default().storage.insert(key, value);
    }

    fn set_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        if contract_address == ContractAddress::default() {
            return Err(StateError::OutOfRangeContractAddress);
        }
        self.contracts.insert(contract_address, class_hash);
        Ok(())
    }

    fn set_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
        compiled_class_hash: CompiledClassHash,
    ) -> StateResult<()> {
        self.classes.entry(class_hash).and_modify(|r| r.compiled_hash = compiled_class_hash);
        Ok(())
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        self.classes.insert(
            *class_hash,
            ClassRecord { class: contract_class, compiled_hash: CompiledClassHash(class_hash.0) },
        );
        Ok(())
    }

    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        let current_nonce = if let Ok(nonce) = self.get_nonce_at(contract_address) {
            nonce
        } else {
            self.db.get_nonce_at(contract_address)?
        };

        let current_nonce_as_u64 = usize::try_from(current_nonce.0)? as u64;
        let next_nonce_val = 1_u64 + current_nonce_as_u64;
        let next_nonce = Nonce(StarkFelt::from(next_nonce_val));
        self.storage.entry(contract_address).or_default().nonce = next_nonce;
        Ok(())
    }

    fn to_state_diff(&self) -> CommitmentStateDiff {
        unreachable!("to_state_diff should not be called on CachedDb")
    }
}

impl<Db> StateReader for CachedDb<Db>
where
    Db: StateExtRef,
{
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        if let Some(value) = self.storage.get(&contract_address).and_then(|r| r.storage.get(&key)) {
            return Ok(*value);
        }

        trace!(target: "cacheddb", "cache miss for storage at address {} index {}", contract_address.0.key(), key.0.key());

        match self.db.get_storage_at(contract_address, key) {
            Ok(value) => {
                trace!(target: "cacheddb", "caching storage at address {} index {}", contract_address.0.key(), key.0.key());
                self.set_storage_at(contract_address, key, value);
                Ok(value)
            }
            Err(err) => Err(err),
        }
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        if let Some(nonce) = self.storage.get(&contract_address).map(|r| r.nonce) {
            return Ok(nonce);
        }

        trace!(target: "cached_db", "cache miss for nonce at {}", contract_address.0.key());

        match self.db.get_nonce_at(contract_address) {
            Ok(nonce) => {
                trace!(target: "cached_db", "caching nonce at {}", contract_address.0.key());
                self.storage.entry(contract_address).or_default().nonce = nonce;
                Ok(nonce)
            }
            Err(err) => Err(err),
        }
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        if let Some(class) = self.classes.get(class_hash).map(|r| r.class.clone()) {
            return Ok(class);
        }

        trace!(target: "cached_db", "cache miss for compiled contract class {class_hash}");

        match self.db.get_compiled_contract_class(class_hash) {
            Ok(class) => {
                trace!(target: "cached_db", "caching compiled contract class {class_hash}");
                self.set_contract_class(class_hash, class.clone())?;
                Ok(class)
            }
            Err(err) => Err(err),
        }
    }

    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        if let Some(class_hash) = self.contracts.get(&contract_address).cloned() {
            return Ok(class_hash);
        }

        trace!(target: "cached_db", "cache miss for class hash at address {}", contract_address.0.key());

        match self.db.get_class_hash_at(contract_address) {
            Ok(class_hash) => {
                trace!(target: "cached_db", "caching class hash at address {}", contract_address.0.key());
                self.set_class_hash_at(contract_address, class_hash)?;
                Ok(class_hash)
            }
            Err(err) => Err(err),
        }
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<starknet_api::core::CompiledClassHash> {
        if let Some(hash) = self.classes.get(&class_hash).map(|r| r.compiled_hash) {
            return Ok(hash);
        }

        trace!(target: "cached_db", "cache miss for compiled class hash {class_hash}");

        match self.db.get_compiled_class_hash(class_hash) {
            Ok(hash) => {
                trace!(target: "cached_db", "caching compiled class hash {class_hash}",);
                self.set_compiled_class_hash(class_hash, hash)?;
                Ok(hash)
            }
            Err(err) => Err(err),
        }
    }
}

impl<Db> StateExt for CachedDb<Db>
where
    Db: StateExtRef,
{
    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()> {
        // check the class hash must not be a legacy contract
        if let ContractClass::V0(_) = self.get_compiled_contract_class(&class_hash)? {
            return Err(StateError::StateReadError("Class hash is not a Sierra class".to_string()));
        };
        self.sierra_classes.insert(class_hash, sierra_class);
        Ok(())
    }
}

impl<Db> StateExtRef for CachedDb<Db>
where
    Db: StateExtRef,
{
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        if let Some(class) = self.sierra_classes.get(class_hash).cloned() {
            return Ok(class);
        }

        trace!(target: "cached_db", "cache miss for sierra class {class_hash}");

        match self.db.get_sierra_class(class_hash) {
            Ok(class) => {
                trace!(target: "cached_db", "caching sierra class {class_hash}");
                self.set_sierra_class(*class_hash, class.clone())?;
                Ok(class)
            }
            Err(err) => Err(err),
        }
    }
}

/// A wrapper type for [CachedState](blockifier::state::cached_state::CachedState) which
/// also allow storing the Sierra classes.
///
/// The inner fields are wrapped in [Arc] and an async [RwLock](tokio::sync::RwLock) as to allow for
/// asynchronous access to the state.
///
/// Example is when it is being referred to as a [StateRefDb] when the 'pending' state is being
/// requested while the block producer also have access to it in order to execute transactions and
/// produce blocks.
#[derive(Debug, Clone)]
pub struct CachedStateWrapper<Db: StateReader> {
    inner: Arc<AsyncRwLock<CachedState<Db>>>,
    sierra_class: Arc<AsyncRwLock<HashMap<ClassHash, FlattenedSierraClass>>>,
}

impl<Db> CachedStateWrapper<Db>
where
    Db: StateExtRef,
{
    pub fn new(db: Db) -> Self {
        Self {
            sierra_class: Default::default(),
            inner: Arc::new(AsyncRwLock::new(CachedState::new(db))),
        }
    }

    pub fn inner_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, CachedState<Db>> {
        tokio::task::block_in_place(|| self.inner.blocking_write())
    }

    pub fn sierra_class(
        &self,
    ) -> tokio::sync::RwLockReadGuard<'_, HashMap<ClassHash, FlattenedSierraClass>> {
        tokio::task::block_in_place(|| self.sierra_class.blocking_read())
    }

    pub fn sierra_class_mut(
        &self,
    ) -> tokio::sync::RwLockWriteGuard<'_, HashMap<ClassHash, FlattenedSierraClass>> {
        tokio::task::block_in_place(|| self.sierra_class.blocking_write())
    }
}

impl<Db> State for CachedStateWrapper<Db>
where
    Db: StateExtRef,
{
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

impl<Db> StateExt for CachedStateWrapper<Db>
where
    Db: StateExtRef,
{
    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()> {
        self.sierra_class_mut().insert(class_hash, sierra_class);
        Ok(())
    }
}

impl<Db> StateReader for CachedStateWrapper<Db>
where
    Db: StateExtRef,
{
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

impl<Db> StateExtRef for CachedStateWrapper<Db>
where
    Db: StateExtRef,
{
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        if let Ok(class) = self.inner_mut().state.get_sierra_class(class_hash) {
            return Ok(class);
        }

        self.sierra_class()
            .get(class_hash)
            .cloned()
            .ok_or(StateError::StateReadError("missing sierra class".to_string()))
    }
}

impl<Db> AsStateRefDb for CachedStateWrapper<Db>
where
    Db: StateExtRef + Clone + Send + Sync + 'static,
{
    fn as_ref_db(&self) -> StateRefDb {
        StateRefDb::new(self.clone())
    }
}

/// Unit tests ported from `blockifier`.
#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use blockifier::state::cached_state::CachedState;
    use starknet_api::core::PatriciaKey;
    use starknet_api::hash::StarkHash;
    use starknet_api::{patricia_key, stark_felt};

    use super::*;
    use crate::backend::in_memory_db::EmptyDb;

    #[test]
    fn get_uninitialized_storage_value() {
        let mut state = CachedState::new(CachedDb::new(EmptyDb));
        let contract_address = ContractAddress(patricia_key!("0x1"));
        let key = StorageKey(patricia_key!("0x10"));
        assert_eq!(state.get_storage_at(contract_address, key).unwrap(), StarkFelt::default());
    }

    #[test]
    fn get_and_set_storage_value() {
        let contract_address0 = ContractAddress(patricia_key!("0x100"));
        let contract_address1 = ContractAddress(patricia_key!("0x200"));
        let key0 = StorageKey(patricia_key!("0x10"));
        let key1 = StorageKey(patricia_key!("0x20"));
        let storage_val0 = stark_felt!("0x1");
        let storage_val1 = stark_felt!("0x5");

        let mut state = CachedState::new(CachedDb {
            contracts: HashMap::from([
                (contract_address0, ClassHash(0_u32.into())),
                (contract_address1, ClassHash(0_u32.into())),
            ]),
            storage: HashMap::from([
                (
                    contract_address0,
                    StorageRecord {
                        nonce: Nonce(0_u32.into()),
                        storage: HashMap::from([(key0, storage_val0)]),
                    },
                ),
                (
                    contract_address1,
                    StorageRecord {
                        nonce: Nonce(0_u32.into()),
                        storage: HashMap::from([(key1, storage_val1)]),
                    },
                ),
            ]),
            classes: HashMap::new(),
            sierra_classes: HashMap::new(),
            db: EmptyDb,
        });

        assert_eq!(state.get_storage_at(contract_address0, key0).unwrap(), storage_val0);
        assert_eq!(state.get_storage_at(contract_address1, key1).unwrap(), storage_val1);

        let modified_storage_value0 = stark_felt!("0xA");
        state.set_storage_at(contract_address0, key0, modified_storage_value0);
        assert_eq!(state.get_storage_at(contract_address0, key0).unwrap(), modified_storage_value0);
        assert_eq!(state.get_storage_at(contract_address1, key1).unwrap(), storage_val1);

        let modified_storage_value1 = stark_felt!("0x7");
        state.set_storage_at(contract_address1, key1, modified_storage_value1);
        assert_eq!(state.get_storage_at(contract_address0, key0).unwrap(), modified_storage_value0);
        assert_eq!(state.get_storage_at(contract_address1, key1).unwrap(), modified_storage_value1);
    }

    #[test]
    fn get_uninitialized_value() {
        let mut state = CachedState::new(CachedDb::new(EmptyDb));
        let contract_address = ContractAddress(patricia_key!("0x1"));
        assert_eq!(state.get_nonce_at(contract_address).unwrap(), Nonce::default());
    }

    #[test]
    fn get_uninitialized_class_hash_value() {
        let mut state = CachedState::new(CachedDb::new(EmptyDb));
        let valid_contract_address = ContractAddress(patricia_key!("0x1"));
        assert_eq!(state.get_class_hash_at(valid_contract_address).unwrap(), ClassHash::default());
    }

    #[test]
    fn cannot_set_class_hash_to_uninitialized_contract() {
        let mut state = CachedState::new(CachedDb::new(EmptyDb));
        let uninitialized_contract_address = ContractAddress::default();
        let class_hash = ClassHash(stark_felt!("0x100"));
        assert_matches!(
            state.set_class_hash_at(uninitialized_contract_address, class_hash).unwrap_err(),
            StateError::OutOfRangeContractAddress
        );
    }

    #[test]
    fn get_and_increment_nonce() {
        let contract_address1 = ContractAddress(patricia_key!("0x100"));
        let contract_address2 = ContractAddress(patricia_key!("0x200"));
        let initial_nonce = Nonce(stark_felt!("0x1"));

        let mut state = CachedState::new(CachedDb {
            contracts: HashMap::from([
                (contract_address1, ClassHash(0_u32.into())),
                (contract_address2, ClassHash(0_u32.into())),
            ]),
            storage: HashMap::from([
                (
                    contract_address1,
                    StorageRecord { nonce: initial_nonce, storage: HashMap::new() },
                ),
                (
                    contract_address2,
                    StorageRecord { nonce: initial_nonce, storage: HashMap::new() },
                ),
            ]),
            classes: HashMap::new(),
            sierra_classes: HashMap::new(),
            db: EmptyDb,
        });

        assert_eq!(state.get_nonce_at(contract_address1).unwrap(), initial_nonce);
        assert_eq!(state.get_nonce_at(contract_address2).unwrap(), initial_nonce);

        assert!(state.increment_nonce(contract_address1).is_ok());
        let nonce1_plus_one = Nonce(stark_felt!("0x2"));
        assert_eq!(state.get_nonce_at(contract_address1).unwrap(), nonce1_plus_one);
        assert_eq!(state.get_nonce_at(contract_address2).unwrap(), initial_nonce);

        assert!(state.increment_nonce(contract_address1).is_ok());
        let nonce1_plus_two = Nonce(stark_felt!("0x3"));
        assert_eq!(state.get_nonce_at(contract_address1).unwrap(), nonce1_plus_two);
        assert_eq!(state.get_nonce_at(contract_address2).unwrap(), initial_nonce);

        assert!(state.increment_nonce(contract_address2).is_ok());
        let nonce2_plus_one = Nonce(stark_felt!("0x2"));
        assert_eq!(state.get_nonce_at(contract_address1).unwrap(), nonce1_plus_two);
        assert_eq!(state.get_nonce_at(contract_address2).unwrap(), nonce2_plus_one);
    }
}
