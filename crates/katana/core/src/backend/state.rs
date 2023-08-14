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
use crate::db::serde::state::{
    SerializableClassRecord, SerializableState, SerializableStorageRecord,
};
use crate::db::Db;

pub trait StateExt {
    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()>;

    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass>;

    fn apply_state<S>(&mut self, state: &mut S)
    where
        S: State + StateReader;
}

#[derive(Clone, Debug, Default)]
pub struct StorageRecord {
    pub nonce: Nonce,
    pub class_hash: ClassHash,
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

#[derive(Clone, Debug)]
pub struct MemDb {
    /// A map of class hash to its class definition.
    pub classes: HashMap<ClassHash, ClassRecord>,
    /// A map of contract address to the contract information.
    pub storage: HashMap<ContractAddress, StorageRecord>,
    /// A map of class hash to its Sierra class definition (if any).
    pub sierra_classes: HashMap<ClassHash, FlattenedSierraClass>,
}

impl Default for MemDb {
    fn default() -> Self {
        let mut state = MemDb {
            storage: HashMap::new(),
            classes: HashMap::new(),
            sierra_classes: HashMap::new(),
        };
        deploy_fee_contract(&mut state);
        deploy_universal_deployer_contract(&mut state);
        state
    }
}

impl StateExt for MemDb {
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        if let ContractClass::V0(_) = self.get_compiled_contract_class(class_hash)? {
            return Err(StateError::StateReadError("Class hash is not a Sierra class".to_string()));
        };

        self.sierra_classes
            .get(class_hash)
            .cloned()
            .ok_or(StateError::StateReadError("Missing Sierra class".to_string()))
    }

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

    fn apply_state<S>(&mut self, state: &mut S)
    where
        S: State + StateReader,
    {
        // Generate the state diff
        let state_diff = state.to_state_diff();

        // update contract storages
        state_diff.storage_updates.into_iter().for_each(|(contract_address, storages)| {
            storages.into_iter().for_each(|(key, value)| {
                self.set_storage_at(contract_address, key, value);
            })
        });

        // update declared contracts
        // apply newly declared classses
        for (class_hash, compiled_class_hash) in &state_diff.class_hash_to_compiled_class_hash {
            let contract_class =
                state.get_compiled_contract_class(class_hash).expect("contract class should exist");
            self.set_contract_class(class_hash, contract_class).unwrap();
            self.set_compiled_class_hash(*class_hash, *compiled_class_hash).unwrap();
        }

        // update deployed contracts
        state_diff.address_to_class_hash.into_iter().for_each(|(contract_address, class_hash)| {
            self.set_class_hash_at(contract_address, class_hash).unwrap()
        });

        // update accounts nonce
        state_diff.address_to_nonce.into_iter().for_each(|(contract_address, nonce)| {
            if let Some(r) = self.storage.get_mut(&contract_address) {
                r.nonce = nonce;
            }
        });
    }
}

impl State for MemDb {
    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        let current_nonce = self.get_nonce_at(contract_address)?;
        let current_nonce_as_u64 = usize::try_from(current_nonce.0)? as u64;
        let next_nonce_val = 1_u64 + current_nonce_as_u64;
        let next_nonce = Nonce(StarkFelt::from(next_nonce_val));
        self.storage.entry(contract_address).or_default().nonce = next_nonce;
        Ok(())
    }

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
        self.storage.entry(contract_address).or_default().class_hash = class_hash;
        Ok(())
    }

    fn set_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
        compiled_class_hash: CompiledClassHash,
    ) -> StateResult<()> {
        if !self.classes.contains_key(&class_hash) {
            return Err(StateError::UndeclaredClassHash(class_hash));
        }
        self.classes.entry(class_hash).and_modify(|r| r.compiled_hash = compiled_class_hash);
        Ok(())
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        let compiled_hash = CompiledClassHash(class_hash.0);
        self.classes.insert(*class_hash, ClassRecord { class: contract_class, compiled_hash });
        Ok(())
    }

    fn to_state_diff(&self) -> CommitmentStateDiff {
        CommitmentStateDiff {
            storage_updates: [].into(),
            address_to_nonce: [].into(),
            address_to_class_hash: [].into(),
            class_hash_to_compiled_class_hash: [].into(),
        }
    }
}

impl StateReader for MemDb {
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        let value = self
            .storage
            .get(&contract_address)
            .and_then(|r| r.storage.get(&key))
            .copied()
            .unwrap_or_default();
        Ok(value)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let nonce = self.storage.get(&contract_address).map(|r| r.nonce).unwrap_or_default();
        Ok(nonce)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.classes
            .get(class_hash)
            .map(|r| r.class.clone())
            .ok_or(StateError::UndeclaredClassHash(*class_hash))
    }

    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        let class_hash =
            self.storage.get(&contract_address).map(|r| r.class_hash).unwrap_or_default();
        Ok(class_hash)
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<starknet_api::core::CompiledClassHash> {
        self.classes
            .get(&class_hash)
            .map(|r| r.compiled_hash)
            .ok_or(StateError::UndeclaredClassHash(class_hash))
    }
}

impl Db for MemDb {
    fn dump_state(&self) -> Result<SerializableState> {
        let mut serializable = SerializableState::default();

        self.storage.iter().for_each(|(addr, storage)| {
            let mut record = SerializableStorageRecord {
                storage: BTreeMap::new(),
                nonce: storage.nonce.0.into(),
                class_hash: storage.class_hash.0.into(),
            };

            storage.storage.iter().for_each(|(key, value)| {
                record.storage.insert((*key.0.key()).into(), (*value).into());
            });

            serializable.storage.insert((*addr.0.key()).into(), record);
        });

        self.classes.iter().for_each(|(class_hash, class_record)| {
            serializable.classes.insert(
                class_hash.0.into(),
                SerializableClassRecord {
                    class: class_record.class.clone().into(),
                    compiled_hash: class_record.compiled_hash.0.into(),
                },
            );
        });

        self.sierra_classes.iter().for_each(|(class_hash, class)| {
            serializable.sierra_classes.insert(class_hash.0.into(), class.clone());
        });

        Ok(serializable)
    }

    fn set_nonce(&mut self, addr: ContractAddress, nonce: Nonce) {
        self.storage.entry(addr).or_default().nonce = nonce;
    }
}

fn deploy_fee_contract(state: &mut MemDb) {
    let address = ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS));
    let hash = ClassHash(*ERC20_CONTRACT_CLASS_HASH);
    let compiled_hash = CompiledClassHash(*ERC20_CONTRACT_CLASS_HASH);

    state.classes.insert(hash, ClassRecord { class: (*ERC20_CONTRACT).clone(), compiled_hash });
    state.storage.insert(
        address,
        StorageRecord { class_hash: hash, nonce: Nonce(1_u128.into()), storage: HashMap::new() },
    );
}

fn deploy_universal_deployer_contract(state: &mut MemDb) {
    let address = ContractAddress(patricia_key!(*UDC_ADDRESS));
    let hash = ClassHash(*UDC_CLASS_HASH);
    let compiled_hash = CompiledClassHash(*UDC_CLASS_HASH);

    state.classes.insert(hash, ClassRecord { class: (*UDC_CONTRACT).clone(), compiled_hash });
    state.storage.insert(
        address,
        StorageRecord { class_hash: hash, nonce: Nonce(1_u128.into()), storage: HashMap::new() },
    );
}

/// Unit tests ported from `blockifier`.
#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use blockifier::state::cached_state::CachedState;
    use starknet_api::core::PatriciaKey;
    use starknet_api::stark_felt;

    use super::*;

    #[test]
    fn get_uninitialized_storage_value() {
        let mut state = CachedState::new(MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        });
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

        let mut state = CachedState::new(MemDb {
            storage: HashMap::from([
                (
                    contract_address0,
                    StorageRecord {
                        class_hash: ClassHash(0_u32.into()),
                        nonce: Nonce(0_u32.into()),
                        storage: HashMap::from([(key0, storage_val0)]),
                    },
                ),
                (
                    contract_address1,
                    StorageRecord {
                        class_hash: ClassHash(0_u32.into()),
                        nonce: Nonce(0_u32.into()),
                        storage: HashMap::from([(key1, storage_val1)]),
                    },
                ),
            ]),
            classes: HashMap::new(),
            sierra_classes: HashMap::new(),
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
        let mut state = CachedState::new(MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        });
        let contract_address = ContractAddress(patricia_key!("0x1"));
        assert_eq!(state.get_nonce_at(contract_address).unwrap(), Nonce::default());
    }

    #[test]
    fn get_uninitialized_class_hash_value() {
        let mut state = CachedState::new(MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        });
        let valid_contract_address = ContractAddress(patricia_key!("0x1"));
        assert_eq!(state.get_class_hash_at(valid_contract_address).unwrap(), ClassHash::default());
    }

    #[test]
    fn cannot_set_class_hash_to_uninitialized_contract() {
        let mut state = CachedState::new(MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        });
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

        let mut state = CachedState::new(MemDb {
            storage: HashMap::from([
                (
                    contract_address1,
                    StorageRecord {
                        class_hash: ClassHash(0_u32.into()),
                        nonce: initial_nonce,
                        storage: HashMap::new(),
                    },
                ),
                (
                    contract_address2,
                    StorageRecord {
                        class_hash: ClassHash(0_u32.into()),
                        nonce: initial_nonce,
                        storage: HashMap::new(),
                    },
                ),
            ]),
            classes: HashMap::new(),
            sierra_classes: HashMap::new(),
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

    #[test]
    fn apply_state_update() {
        let mut old_state = MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        };
        let mut new_state = CachedState::new(MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        });

        let class_hash = ClassHash(stark_felt!("0x1"));
        let address = ContractAddress(patricia_key!("0x1"));
        let storage_key = StorageKey(patricia_key!("0x77"));
        let storage_val = stark_felt!("0x66");
        let contract = (*UDC_CONTRACT).clone();
        let compiled_hash = CompiledClassHash(class_hash.0);

        new_state.set_contract_class(&class_hash, (*UDC_CONTRACT).clone()).unwrap();
        new_state.set_compiled_class_hash(class_hash, CompiledClassHash(class_hash.0)).unwrap();
        new_state.set_class_hash_at(address, class_hash).unwrap();
        new_state.set_storage_at(address, storage_key, storage_val);

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

        old_state.apply_state(&mut new_state);

        assert_eq!(old_state.get_compiled_contract_class(&class_hash).unwrap(), contract);
        assert_eq!(old_state.get_compiled_class_hash(class_hash).unwrap(), compiled_hash);
        assert_eq!(old_state.get_class_hash_at(address).unwrap(), class_hash);
        assert_eq!(old_state.get_storage_at(address, storage_key).unwrap(), storage_val);
    }

    #[test]
    fn dump_and_load_state() {
        let mut state = MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        };

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

        let mut new_state = MemDb {
            classes: HashMap::new(),
            storage: HashMap::new(),
            sierra_classes: HashMap::new(),
        };
        new_state.load_state(dump).expect("should load state");

        assert_eq!(new_state.get_compiled_contract_class(&class_hash).unwrap(), contract);
        assert_eq!(new_state.get_compiled_class_hash(class_hash).unwrap(), compiled_hash);
        assert_eq!(new_state.get_class_hash_at(address).unwrap(), class_hash);
        assert_eq!(new_state.get_storage_at(address, storage_key).unwrap(), storage_val);
    }
}
