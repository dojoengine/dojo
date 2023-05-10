use blockifier::execution::contract_class::ContractClass;
use blockifier::state::cached_state::ContractStorageKey;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::StateReader;
use blockifier::state::state_api::StateResult;
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey},
    hash::{StarkFelt, StarkHash},
    patricia_key, stark_felt,
    state::StorageKey,
};
use std::collections::HashMap;

use crate::constants::ERC20_CONTRACT_CLASS_HASH;
use crate::constants::ERC20_CONTRACT_PATH;
use crate::constants::FEE_ERC20_CONTRACT_ADDRESS;
use crate::constants::UNIVERSAL_DEPLOYER_CLASS_HASH;
use crate::constants::UNIVERSAL_DEPLOYER_CONTRACT_ADDRESS;
use crate::constants::UNIVERSAL_DEPLOYER_CONTRACT_PATH;
use crate::util::get_contract_class;

#[derive(Clone, Debug)]
pub struct DictStateReader {
    pub storage_view: HashMap<ContractStorageKey, StarkFelt>,
    pub address_to_nonce: HashMap<ContractAddress, Nonce>,
    pub address_to_class_hash: HashMap<ContractAddress, ClassHash>,
    pub class_hash_to_class: HashMap<ClassHash, ContractClass>,
    pub class_hash_to_compiled_class_hash: HashMap<ClassHash, CompiledClassHash>,
}

impl Default for DictStateReader {
    fn default() -> Self {
        let mut state = DictStateReader {
            storage_view: HashMap::new(),
            address_to_nonce: HashMap::new(),
            address_to_class_hash: HashMap::new(),
            class_hash_to_class: HashMap::new(),
            class_hash_to_compiled_class_hash: HashMap::new(),
        };
        deploy_fee_contract(&mut state);
        deploy_universal_deployer_contract(&mut state);
        state
    }
}

impl StateReader for DictStateReader {
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        let contract_storage_key = (contract_address, key);
        let value = self
            .storage_view
            .get(&contract_storage_key)
            .copied()
            .unwrap_or_default();
        Ok(value)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let nonce = self
            .address_to_nonce
            .get(&contract_address)
            .copied()
            .unwrap_or_default();
        Ok(nonce)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        let contract_class = self.class_hash_to_class.get(class_hash).cloned();
        match contract_class {
            Some(contract_class) => Ok(contract_class),
            None => Err(StateError::UndeclaredClassHash(*class_hash)),
        }
    }

    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        let class_hash = self
            .address_to_class_hash
            .get(&contract_address)
            .copied()
            .unwrap_or_default();
        Ok(class_hash)
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<starknet_api::core::CompiledClassHash> {
        let compiled_class_hash = self
            .class_hash_to_compiled_class_hash
            .get(&class_hash)
            .copied()
            .unwrap_or_default();
        Ok(compiled_class_hash)
    }
}

fn deploy_fee_contract(state: &mut DictStateReader) {
    let erc20_class_hash = ClassHash(stark_felt!(ERC20_CONTRACT_CLASS_HASH));
    state
        .class_hash_to_class
        .insert(erc20_class_hash, get_contract_class(ERC20_CONTRACT_PATH));
    state.address_to_class_hash.insert(
        ContractAddress(patricia_key!(FEE_ERC20_CONTRACT_ADDRESS)),
        erc20_class_hash,
    );
}

fn deploy_universal_deployer_contract(state: &mut DictStateReader) {
    let universal_deployer_class_hash = ClassHash(stark_felt!(UNIVERSAL_DEPLOYER_CLASS_HASH));
    state.class_hash_to_class.insert(
        universal_deployer_class_hash,
        get_contract_class(UNIVERSAL_DEPLOYER_CONTRACT_PATH),
    );
    state.address_to_class_hash.insert(
        ContractAddress(patricia_key!(UNIVERSAL_DEPLOYER_CONTRACT_ADDRESS)),
        universal_deployer_class_hash,
    );
}
