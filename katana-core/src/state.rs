use blockifier::execution::contract_class::ContractClass;
use blockifier::state::cached_state::ContractStorageKey;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::StateReader;
use blockifier::state::state_api::StateResult;
use starknet_api::patricia_key;
use starknet_api::stark_felt;
use starknet_api::state::StorageKey;
use starknet_api::{
    core::{ClassHash, ContractAddress, Nonce, PatriciaKey},
    hash::{StarkFelt, StarkHash},
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::util::get_contract_class;
use crate::{FEE_ERC20_CONTRACT_ADDRESS, UNIVERSAL_DEPLOYER_CONTRACT_ADDRESS};

pub const ACCOUNT_CONTRACT_CLASS_HASH: &str = "0x100";
pub const ERC20_CONTRACT_CLASS_HASH: &str = "0x200";
pub const UNIVERSAL_DEPLOYER_CLASS_HASH: &str = "0x300";

pub const ACCOUNT_CONTRACT_PATH: &str = "contracts/compiled/account.json";
pub const ERC20_CONTRACT_PATH: &str = "./contracts/compiled/erc20.json";
pub const UNIVERSAL_DEPLOYER_CONTRACT_PATH: &str = "./contracts/compiled/universal_deployer.json";

#[derive(Clone, Debug, Default)]
pub struct DictStateReader {
    pub storage_view: HashMap<ContractStorageKey, StarkFelt>,
    pub address_to_nonce: HashMap<ContractAddress, Nonce>,
    pub address_to_class_hash: HashMap<ContractAddress, ClassHash>,
    pub class_hash_to_class: HashMap<ClassHash, ContractClass>,
}

impl DictStateReader {
    pub fn new() -> Self {
        // Declare all the needed contracts.
        let account_class_hash = ClassHash(stark_felt!(ACCOUNT_CONTRACT_CLASS_HASH));
        let erc20_class_hash = ClassHash(stark_felt!(ERC20_CONTRACT_CLASS_HASH));
        let universal_deployer_class_hash = ClassHash(stark_felt!(UNIVERSAL_DEPLOYER_CLASS_HASH));

        let class_hash_to_class: HashMap<ClassHash, ContractClass> = HashMap::from([
            (
                account_class_hash,
                get_contract_class(ACCOUNT_CONTRACT_PATH),
            ),
            (erc20_class_hash, get_contract_class(ERC20_CONTRACT_PATH)),
            (
                universal_deployer_class_hash,
                get_contract_class(UNIVERSAL_DEPLOYER_CONTRACT_PATH),
            ),
        ]);

        let address_to_class_hash = HashMap::from([
            (
                ContractAddress(patricia_key!(FEE_ERC20_CONTRACT_ADDRESS)),
                erc20_class_hash,
            ),
            (
                ContractAddress(patricia_key!(UNIVERSAL_DEPLOYER_CONTRACT_ADDRESS)),
                universal_deployer_class_hash,
            ),
        ]);

        Self {
            address_to_class_hash,
            class_hash_to_class,
            ..Default::default()
        }
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

    fn get_contract_class(&mut self, class_hash: &ClassHash) -> StateResult<Arc<ContractClass>> {
        let contract_class = self.class_hash_to_class.get(class_hash).cloned();
        match contract_class {
            Some(contract_class) => Ok(Arc::from(contract_class)),
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
}
