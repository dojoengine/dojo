use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, ContractClass};

use crate::ProviderResult;

/// A provider trait for retrieving contract class related information.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassProvider: Send + Sync {
    /// Returns the compiled class definition of a contract class given its class hash.
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<ContractClass>>;

    /// Returns the compiled class definition of a contract class given its class hash.
    fn compiled_class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>>;

    /// Returns the compiled class hash for the given class hash.
    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>>;
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
    fn set_class(&self, hash: ClassHash, class: ContractClass) -> ProviderResult<()>;

    /// Retrieves the Sierra class definition of a contract class given its class hash.
    fn set_compiled_class(&self, hash: ClassHash, class: CompiledClass) -> ProviderResult<()>;
}
