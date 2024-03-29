use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, GenericContractInfo};

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractInfoProvider: Send + Sync {
    /// Returns the contract information given its address.
    fn contract(&self, address: ContractAddress) -> ProviderResult<Option<GenericContractInfo>>;
}

/// A provider trait for retrieving contract class related information.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassProvider: Send + Sync {
    /// Returns the compiled class hash for the given class hash.
    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>>;

    /// Returns the compiled class definition of a contract class given its class hash.
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>>;

    /// Retrieves the Sierra class definition of a contract class given its class hash.
    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>>;
}

// TEMP: added mainly for compatibility reason. might be removed in the future.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassWriter: Send + Sync {
    /// Returns the compiled class hash for the given class hash.
    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> ProviderResult<()>;

    /// Returns the compiled class definition of a contract class given its class hash.
    fn set_class(&self, hash: ClassHash, class: CompiledClass) -> ProviderResult<()>;

    /// Retrieves the Sierra class definition of a contract class given its class hash.
    fn set_sierra_class(&self, hash: ClassHash, sierra: FlattenedSierraClass)
    -> ProviderResult<()>;
}
