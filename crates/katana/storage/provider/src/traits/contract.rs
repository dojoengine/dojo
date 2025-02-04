use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, ContractClass};

use crate::ProviderResult;

/// A provider trait for retrieving contract class related information.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassProvider {
    /// Returns the compiled class definition of a contract class given its class hash.
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<ContractClass>>;

    /// Returns the compiled class hash for the given class hash.
    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassWriter {
    /// Sets the compiled class hash for the given class hash.
    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> ProviderResult<()>;

    /// Sets the contract class for the given class hash.
    fn set_class(&self, hash: ClassHash, class: ContractClass) -> ProviderResult<()>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassWriterExt: ContractClassWriter {
    /// Set the compiled class for the given class hash.
    fn set_compiled_class(&self, hash: ClassHash, class: CompiledClass) -> ProviderResult<()>;
}

pub trait ContractClassProviderExt: ContractClassProvider {
    /// Returns the compiled class definition of a contract class given its class hash.
    ///
    /// It depends on the provider implementation on how to store/manage the compiled classes, be it
    /// compiling on demand (default implementation), or storing the compiled class in the database
    /// or volatile cache.
    fn compiled_class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        if let Some(class) = self.class(hash)? { Ok(Some(class.compile()?)) } else { Ok(None) }
    }
}

impl<T: ContractClassProvider> ContractClassProviderExt for T {}
