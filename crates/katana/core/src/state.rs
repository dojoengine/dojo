use std::collections::HashMap;

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

pub trait StateExt {
    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()>;

    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass>;
}

#[derive(Clone, Debug, Default)]
pub struct StorageRecord {
    pub nonce: Nonce,
    pub class_hash: ClassHash,
    pub storage: HashMap<StorageKey, StarkFelt>,
}

#[derive(Clone, Debug)]
pub struct ClassRecord {
    pub class: ContractClass,
    /// The hash of a compiled Sierra class (if the class is a Sierra class, otherwise
    /// for legacy contract, it is the same as the class hash).
    pub compiled_hash: CompiledClassHash,
    pub sierra_class: Option<FlattenedSierraClass>,
}

#[derive(Clone, Debug)]
pub struct MemDb {
    pub state: HashMap<ContractAddress, StorageRecord>,
    pub classes: HashMap<ClassHash, ClassRecord>,
}

impl Default for MemDb {
    fn default() -> Self {
        let mut state = MemDb { state: HashMap::new(), classes: HashMap::new() };
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

        self.classes
            .get(class_hash)
            .and_then(|r| r.sierra_class.clone())
            .ok_or(StateError::StateReadError("Missing Sierra class".to_string()))
    }

    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()> {
        self.classes.get_mut(&class_hash).map(|r| r.sierra_class = Some(sierra_class));
        Ok(())
    }
}

impl State for MemDb {
    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        let current_nonce = self.get_nonce_at(contract_address)?;
        let current_nonce_as_u64 = usize::try_from(current_nonce.0)? as u64;
        let next_nonce_val = 1_u64 + current_nonce_as_u64;
        let next_nonce = Nonce(StarkFelt::from(next_nonce_val));
        self.state.entry(contract_address).or_default().nonce = next_nonce;
        Ok(())
    }

    fn set_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
        value: StarkFelt,
    ) {
        self.state.entry(contract_address).or_default().storage.insert(key, value);
    }

    fn set_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        if contract_address == ContractAddress::default() {
            return Err(StateError::OutOfRangeContractAddress);
        }
        self.state.entry(contract_address).or_default().class_hash = class_hash;
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
        self.classes.insert(
            *class_hash,
            ClassRecord {
                sierra_class: None,
                class: contract_class,
                compiled_hash: CompiledClassHash(class_hash.0),
            },
        );
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
            .state
            .get(&contract_address)
            .and_then(|r| r.storage.get(&key))
            .copied()
            .unwrap_or_default();
        Ok(value)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let nonce = self.state.get(&contract_address).map(|r| r.nonce).unwrap_or_default();
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
            self.state.get(&contract_address).map(|r| r.class_hash).unwrap_or_default();
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

fn deploy_fee_contract(state: &mut MemDb) {
    let address = ContractAddress(patricia_key!(*FEE_TOKEN_ADDRESS));
    let hash = ClassHash(*ERC20_CONTRACT_CLASS_HASH);
    let compiled_hash = CompiledClassHash(*ERC20_CONTRACT_CLASS_HASH);

    state.classes.insert(
        hash,
        ClassRecord { sierra_class: None, class: (*ERC20_CONTRACT).clone(), compiled_hash },
    );

    state.state.insert(
        address,
        StorageRecord { class_hash: hash, nonce: Nonce(1_u128.into()), storage: HashMap::new() },
    );
}

fn deploy_universal_deployer_contract(state: &mut MemDb) {
    let address = ContractAddress(patricia_key!(*UDC_ADDRESS));
    let hash = ClassHash(*UDC_CLASS_HASH);
    let compiled_hash = CompiledClassHash(*UDC_CLASS_HASH);

    state.classes.insert(
        hash,
        ClassRecord { sierra_class: None, class: (*UDC_CONTRACT).clone(), compiled_hash },
    );

    state.state.insert(
        address,
        StorageRecord { class_hash: hash, nonce: Nonce(1_u128.into()), storage: HashMap::new() },
    );
}

/// Unit tests ported from `blockifier`.
#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use blockifier::state::cached_state::CachedState;
    use starknet_api::stark_felt;

    use super::*;

    #[test]
    fn get_uninitialized_storage_value() {
        let mut state = CachedState::new(MemDb { classes: HashMap::new(), state: HashMap::new() });
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
            state: HashMap::from([
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
        let mut state = CachedState::new(MemDb { classes: HashMap::new(), state: HashMap::new() });
        let contract_address = ContractAddress(patricia_key!("0x1"));
        assert_eq!(state.get_nonce_at(contract_address).unwrap(), Nonce::default());
    }

    #[test]
    fn get_uninitialized_class_hash_value() {
        let mut state = CachedState::new(MemDb { classes: HashMap::new(), state: HashMap::new() });
        let valid_contract_address = ContractAddress(patricia_key!("0x1"));
        assert_eq!(state.get_class_hash_at(valid_contract_address).unwrap(), ClassHash::default());
    }

    #[test]
    fn cannot_set_class_hash_to_uninitialized_contract() {
        let mut state = CachedState::new(MemDb { classes: HashMap::new(), state: HashMap::new() });
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
            state: HashMap::from([
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
