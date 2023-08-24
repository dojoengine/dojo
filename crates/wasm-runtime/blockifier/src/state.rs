// use std::collections::HashMap;
use blockifier::execution::contract_class::ContractClass;
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{StateReader, StateResult};
use blockifier::stdlib::collections::HashMap;
use starknet_api::api_core::{ClassHash, CompiledClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use crate::utils::addr;

#[derive(Debug, Default)]
pub struct ClientState {
    pub contracts: HashMap<ContractAddress, (ClassHash, Nonce, HashMap<StorageKey, StarkFelt>)>,
    pub classes: HashMap<ClassHash, ContractClass>,
}

/// A read-only API for accessing StarkNet global state.
///
/// The `self` argument is mutable for flexibility during reads (for example, caching reads),
/// and to allow for the `State` trait below to also be considered a `StateReader`.
impl StateReader for ClientState {
    /// Returns the storage value under the given key in the given contract instance (represented by
    /// its address).
    /// Default: 0 for an uninitialized contract address.
    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        let contract_entry_opt = self.contracts.get(&contract_address);
        let value = match contract_entry_opt {
            Some((_, _, storage)) => match storage.get(&key) {
                Some(v) => *v,
                None => addr::felt("0"),
            },
            None => {
                // return StateResult::Err(StateError::UnavailableContractAddress(contract_address));
                addr::felt("0")
            }
        };

        StateResult::Ok(value)
    }

    /// Returns the nonce of the given contract instance.
    /// Default: 0 for an uninitialized contract address.
    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let contract_entry_opt = self.contracts.get(&contract_address);
        match contract_entry_opt {
            Some((_, nonce, _)) => StateResult::Ok(*nonce),
            None => StateResult::Err(StateError::UnavailableContractAddress(contract_address)),
        }
    }

    /// Returns the class hash of the contract class at the given contract instance.
    /// Default: 0 (uninitialized class hash) for an uninitialized contract address.
    fn get_class_hash_at(&mut self, contract_address: ContractAddress) -> StateResult<ClassHash> {
        let contract_entry_opt = self.contracts.get(&contract_address);
        match contract_entry_opt {
            Some((class_hash, _, _)) => StateResult::Ok(*class_hash),
            None => StateResult::Err(StateError::UnavailableContractAddress(contract_address)),
        }
    }

    /// Returns the contract class of the given class hash.
    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        let contract_class_opt = self.classes.get(class_hash);
        let Some(classes) = contract_class_opt else {
            return StateResult::Err(StateError::UndeclaredClassHash(*class_hash));
        };
        StateResult::Ok(classes.clone())
    }

    /// Returns the compiled class hash of the given class hash.
    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        StateResult::Ok(CompiledClassHash(class_hash.0))
    }
}
