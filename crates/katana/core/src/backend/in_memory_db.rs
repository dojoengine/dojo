use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use blockifier::execution::contract_class::ContractClass;
use blockifier::state::cached_state::CommitmentStateDiff;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{State, StateReader, StateResult};
use starknet::core::types::FlattenedSierraClass;
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;

use crate::constants::{
    ERC20_CONTRACT, ERC20_CONTRACT_CLASS_HASH, FEE_TOKEN_ADDRESS, UDC_ADDRESS, UDC_CLASS_HASH,
    UDC_CONTRACT,
};
use crate::db::cached::{CachedDb, ClassRecord, StorageRecord};
use crate::db::serde::state::{
    SerializableClassRecord, SerializableState, SerializableStorageRecord,
};
use crate::db::{AsStateRefDb, Database, StateExt, StateExtRef, StateRefDb};

/// An empty state database which returns default values for all queries.
#[derive(Debug, Clone)]
pub struct EmptyDb;

impl StateReader for EmptyDb {
    fn get_class_hash_at(&mut self, _contract_address: ContractAddress) -> StateResult<ClassHash> {
        Ok(ClassHash::default())
    }

    fn get_nonce_at(&mut self, _contract_address: ContractAddress) -> StateResult<Nonce> {
        Ok(Nonce::default())
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        Err(StateError::UndeclaredClassHash(class_hash))
    }

    fn get_storage_at(
        &mut self,
        _contract_address: ContractAddress,
        _key: StorageKey,
    ) -> StateResult<StarkFelt> {
        Ok(StarkFelt::default())
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        Err(StateError::UndeclaredClassHash(*class_hash))
    }
}

impl StateExtRef for EmptyDb {
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        Err(StateError::UndeclaredClassHash(*class_hash))
    }
}

/// A in memory state database implementation with empty cache db.
#[derive(Clone, Debug)]
pub struct MemDb {
    pub db: CachedDb<EmptyDb>,
}

impl MemDb {
    pub fn new() -> Self {
        Self { db: CachedDb::new(EmptyDb) }
    }
}

impl Default for MemDb {
    fn default() -> Self {
        let mut state = Self::new();
        deploy_fee_contract(&mut state);
        deploy_universal_deployer_contract(&mut state);
        state
    }
}

impl State for MemDb {
    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        let current_nonce = self.get_nonce_at(contract_address)?;
        let current_nonce_as_u64 = usize::try_from(current_nonce.0)? as u64;
        let next_nonce_val = 1_u64 + current_nonce_as_u64;
        let next_nonce = Nonce(StarkFelt::from(next_nonce_val));
        self.db.storage.entry(contract_address).or_default().nonce = next_nonce;
        Ok(())
    }

    fn set_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
        value: StarkFelt,
    ) {
        self.db.storage.entry(contract_address).or_default().storage.insert(key, value);
    }

    fn set_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        if contract_address == ContractAddress::default() {
            return Err(StateError::OutOfRangeContractAddress);
        }
        self.db.contracts.insert(contract_address, class_hash);
        Ok(())
    }

    fn set_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
        compiled_class_hash: CompiledClassHash,
    ) -> StateResult<()> {
        if !self.db.classes.contains_key(&class_hash) {
            return Err(StateError::UndeclaredClassHash(class_hash));
        }
        self.db.classes.entry(class_hash).and_modify(|r| r.compiled_hash = compiled_class_hash);
        Ok(())
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        let compiled_hash = CompiledClassHash(class_hash.0);
        self.db.classes.insert(*class_hash, ClassRecord { class: contract_class, compiled_hash });
        Ok(())
    }

    fn to_state_diff(&self) -> CommitmentStateDiff {
        unreachable!("to_state_diff should not be called on MemDb")
    }
}

impl StateReader for MemDb {
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        let value = self
            .db
            .storage
            .get(&contract_address)
            .and_then(|r| r.storage.get(&key))
            .copied()
            .unwrap_or_default();
        Ok(value)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let nonce = self.db.storage.get(&contract_address).map(|r| r.nonce).unwrap_or_default();
        Ok(nonce)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.db
            .classes
            .get(class_hash)
            .map(|r| r.class.clone())
            .ok_or(StateError::UndeclaredClassHash(*class_hash))
    }

    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        Ok(self.db.contracts.get(&contract_address).cloned().unwrap_or_default())
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<starknet_api::core::CompiledClassHash> {
        self.db
            .classes
            .get(&class_hash)
            .map(|r| r.compiled_hash)
            .ok_or(StateError::UndeclaredClassHash(class_hash))
    }
}

impl StateExtRef for MemDb {
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        if let ContractClass::V0(_) = self.get_compiled_contract_class(class_hash)? {
            return Err(StateError::StateReadError("Class hash is not a Sierra class".to_string()));
        };

        self.db
            .sierra_classes
            .get(class_hash)
            .cloned()
            .ok_or(StateError::StateReadError("Missing Sierra class".to_string()))
    }
}

impl StateExt for MemDb {
    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()> {
        // check the class hash must not be a legacy contract
        if let ContractClass::V0(_) = self.get_compiled_contract_class(&class_hash)? {
            return Err(StateError::StateReadError("Class hash is not a Sierra class".to_string()));
        };
        self.db.sierra_classes.insert(class_hash, sierra_class);
        Ok(())
    }
}

impl AsStateRefDb for MemDb {
    fn as_ref_db(&self) -> StateRefDb {
        StateRefDb::new(MemDb { db: self.db.clone() })
    }
}

impl Database for MemDb {
    fn dump_state(&self) -> Result<SerializableState> {
        let mut serializable = SerializableState::default();

        self.db.storage.iter().for_each(|(addr, storage)| {
            let mut record = SerializableStorageRecord {
                storage: BTreeMap::new(),
                nonce: storage.nonce.0.into(),
            };

            storage.storage.iter().for_each(|(key, value)| {
                record.storage.insert((*key.0.key()).into(), (*value).into());
            });

            serializable.storage.insert((*addr.0.key()).into(), record);
        });

        self.db.classes.iter().for_each(|(class_hash, class_record)| {
            serializable.classes.insert(
                class_hash.0.into(),
                SerializableClassRecord {
                    class: class_record.class.clone().into(),
                    compiled_hash: class_record.compiled_hash.0.into(),
                },
            );
        });

        self.db.contracts.iter().for_each(|(address, class_hash)| {
            serializable.contracts.insert((*address.0.key()).into(), class_hash.0.into());
        });

        self.db.sierra_classes.iter().for_each(|(class_hash, class)| {
            serializable.sierra_classes.insert(class_hash.0.into(), class.clone());
        });

        Ok(serializable)
    }

    fn set_nonce(&mut self, addr: ContractAddress, nonce: Nonce) {
        self.db.storage.entry(addr).or_default().nonce = nonce;
    }
}

fn deploy_fee_contract(state: &mut MemDb) {
    let address = ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS));
    let hash = ClassHash(*ERC20_CONTRACT_CLASS_HASH);
    let compiled_hash = CompiledClassHash(*ERC20_CONTRACT_CLASS_HASH);

    state.db.classes.insert(hash, ClassRecord { class: (*ERC20_CONTRACT).clone(), compiled_hash });
    state.db.contracts.insert(address, hash);
    state
        .db
        .storage
        .insert(address, StorageRecord { nonce: Nonce(1_u128.into()), storage: HashMap::new() });
}

fn deploy_universal_deployer_contract(state: &mut MemDb) {
    let address = ContractAddress(patricia_key!(*UDC_ADDRESS));
    let hash = ClassHash(*UDC_CLASS_HASH);
    let compiled_hash = CompiledClassHash(*UDC_CLASS_HASH);

    state.db.classes.insert(hash, ClassRecord { class: (*UDC_CONTRACT).clone(), compiled_hash });
    state.db.contracts.insert(address, hash);
    state
        .db
        .storage
        .insert(address, StorageRecord { nonce: Nonce(1_u128.into()), storage: HashMap::new() });
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use starknet_api::core::{ClassHash, PatriciaKey};
    use starknet_api::stark_felt;

    use super::*;
    use crate::backend::in_memory_db::MemDb;
    use crate::constants::UDC_CONTRACT;
    // use crate::db::cached::CachedStateWrapper;
    use crate::execution::ExecutionOutcome;

    #[test]
    fn dump_and_load_state() {
        let mut state = MemDb::new();

        let class_hash = ClassHash(stark_felt!("0x1"));
        let address = ContractAddress(patricia_key!("0x1"));
        let storage_key = StorageKey(patricia_key!("0x77"));
        let storage_val = stark_felt!("0x66");
        let contract = (*UDC_CONTRACT).clone();
        let compiled_hash = CompiledClassHash(class_hash.0);

        state.set_contract_class(&class_hash, (*UDC_CONTRACT).clone()).unwrap();
        state.set_compiled_class_hash(class_hash, CompiledClassHash(class_hash.0)).unwrap();
        state.set_class_hash_at(address, class_hash).unwrap();
        state.set_storage_at(address, storage_key, storage_val);

        let dump = state.dump_state().expect("should dump state");

        let mut new_state = MemDb::new();
        new_state.load_state(dump).expect("should load state");

        assert_eq!(new_state.get_compiled_contract_class(&class_hash).unwrap(), contract);
        assert_eq!(new_state.get_compiled_class_hash(class_hash).unwrap(), compiled_hash);
        assert_eq!(new_state.get_class_hash_at(address).unwrap(), class_hash);
        assert_eq!(new_state.get_storage_at(address, storage_key).unwrap(), storage_val);
    }

    #[test]
    fn apply_state_update() {
        let mut old_state = MemDb::new();

        let class_hash = ClassHash(stark_felt!("0x1"));
        let address = ContractAddress(patricia_key!("0x1"));
        let storage_key = StorageKey(patricia_key!("0x77"));
        let storage_val = stark_felt!("0x66");
        let contract = (*UDC_CONTRACT).clone();
        let compiled_hash = CompiledClassHash(class_hash.0);

        let execution_outcome = ExecutionOutcome {
            state_diff: CommitmentStateDiff {
                address_to_class_hash: [(address, class_hash)].into(),
                address_to_nonce: [].into(),
                storage_updates: [(address, [(storage_key, storage_val)].into())].into(),
                class_hash_to_compiled_class_hash: [(class_hash, CompiledClassHash(class_hash.0))]
                    .into(),
            },
            transactions: vec![],
            declared_classes: HashMap::from([(class_hash, (*UDC_CONTRACT).clone())]),
            declared_sierra_classes: HashMap::new(),
        };

        assert_matches!(
            old_state.get_compiled_contract_class(&class_hash),
            Err(StateError::UndeclaredClassHash(_))
        );
        assert_matches!(
            old_state.get_compiled_class_hash(class_hash),
            Err(StateError::UndeclaredClassHash(_))
        );
        assert_eq!(old_state.get_class_hash_at(address).unwrap(), ClassHash::default());
        assert_eq!(old_state.get_storage_at(address, storage_key).unwrap(), StarkFelt::default());

        execution_outcome.apply_to(&mut old_state);

        assert_eq!(old_state.get_compiled_contract_class(&class_hash).unwrap(), contract);
        assert_eq!(old_state.get_compiled_class_hash(class_hash).unwrap(), compiled_hash);
        assert_eq!(old_state.get_class_hash_at(address).unwrap(), class_hash);
        assert_eq!(old_state.get_storage_at(address, storage_key).unwrap(), storage_val);
    }
}
