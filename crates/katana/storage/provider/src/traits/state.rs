use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::class::ClassHash;
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::Felt;
use katana_trie::MultiProof;
use starknet::macros::short_string;
use starknet_types_core::hash::StarkHash;

use super::contract::ContractClassProvider;
use crate::error::ProviderError;
use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateRootProvider: Send + Sync {
    /// Retrieves the root of the global state trie.
    fn state_root(&self) -> ProviderResult<Felt> {
        // https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-state/#state_commitment
        Ok(starknet_types_core::hash::Poseidon::hash_array(&[
            short_string!("STARKNET_STATE_V0"),
            self.contracts_root()?,
            self.classes_root()?,
        ]))
    }

    /// Retrieves the root of the classes trie.
    fn classes_root(&self) -> ProviderResult<Felt> {
        Err(ProviderError::StateRootNotFound)
    }

    /// Retrieves the root of the contracts trie.
    fn contracts_root(&self) -> ProviderResult<Felt> {
        Err(ProviderError::StateRootNotFound)
    }

    /// Retrieves the root of a contract's storage trie.
    fn storage_root(&self, contract: ContractAddress) -> ProviderResult<Option<Felt>> {
        let _ = contract;
        Ok(None)
    }
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateProvider:
    ContractClassProvider + StateProofProvider + StateRootProvider + Send + Sync + std::fmt::Debug
{
    /// Returns the nonce of a contract.
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>>;

    /// Returns the value of a contract storage.
    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>>;

    /// Returns the class hash of a contract.
    fn class_hash_of_contract(&self, address: ContractAddress)
    -> ProviderResult<Option<ClassHash>>;
}

/// A type which can create [`StateProvider`] for states at a particular block.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateFactoryProvider: Send + Sync {
    /// Returns a state provider for retrieving the latest state.
    fn latest(&self) -> ProviderResult<Box<dyn StateProvider>>;

    /// Returns a state provider for retrieving historical state at the given block.
    fn historical(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Box<dyn StateProvider>>>;
}

// TEMP: added mainly for compatibility reason. it might be removed in the future.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateWriter: Send + Sync {
    /// Sets the nonce of a contract.
    fn set_nonce(&self, address: ContractAddress, nonce: Nonce) -> ProviderResult<()>;

    /// Sets the value of a contract storage.
    fn set_storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
        storage_value: StorageValue,
    ) -> ProviderResult<()>;

    /// Sets the class hash of a contract.
    fn set_class_hash_of_contract(
        &self,
        address: ContractAddress,
        class_hash: ClassHash,
    ) -> ProviderResult<()>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateProofProvider: Send + Sync {
    fn storage_multiproof(
        &self,
        address: ContractAddress,
        key: Vec<StorageKey>,
    ) -> ProviderResult<MultiProof> {
        let _ = address;
        let _ = key;
        Err(ProviderError::StateProofNotSupported)
    }

    fn contract_multiproof(&self, addresses: Vec<ContractAddress>) -> ProviderResult<MultiProof> {
        let _ = addresses;
        Err(ProviderError::StateProofNotSupported)
    }

    fn class_multiproof(&self, classes: Vec<ClassHash>) -> ProviderResult<MultiProof> {
        let _ = classes;
        Err(ProviderError::StateProofNotSupported)
    }
}
