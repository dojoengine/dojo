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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_new_storage_record() {
        let mut state = MemDb { classes: HashMap::new(), state: HashMap::new() };

        let contract_address = ContractAddress(patricia_key!(0x1234_u32));
        let key = StorageKey(patricia_key!(0x5678_u32));
        let value = StarkFelt::from(0x9abc_u32);

        state.set_storage_at(contract_address, key, value);
        let actual = state.get_storage_at(contract_address, key).unwrap();

        assert_eq!(actual, value);
    }

    #[test]
    fn set_new_storage_should_overwrite_old_value() {
        let mut state = MemDb { classes: HashMap::new(), state: HashMap::new() };

        let contract_address = ContractAddress(patricia_key!(0x1234_u32));
        let key = StorageKey(patricia_key!(0x5678_u32));
        let value = StarkFelt::from(0x9abc_u32);

        state.set_storage_at(contract_address, key, value);
        let actual = state.get_storage_at(contract_address, key).unwrap();

        assert_eq!(actual, value);

        let new_value = StarkFelt::from(0xdef0_u32);
        state.set_storage_at(contract_address, key, new_value);
        let actual = state.get_storage_at(contract_address, key).unwrap();

        assert_eq!(actual, new_value);
    }

    #[test]
    fn set_compiled_hash_for_undeclared_class_hash_should_fail() {
        let mut state = MemDb { classes: HashMap::new(), state: HashMap::new() };
        let class_hash = ClassHash(*UDC_CLASS_HASH);
        let compiled_class_hash = CompiledClassHash(*UDC_CLASS_HASH);
        assert!(state.set_compiled_class_hash(class_hash, compiled_class_hash).is_err());
    }
}
