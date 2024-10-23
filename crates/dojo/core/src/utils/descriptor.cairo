//! Descriptor is used to verify the consistency of the selector from the namespace and the name.
use core::num::traits::Zero;
use core::panics::panic_with_byte_array;
use core::poseidon::poseidon_hash_span;
use dojo::utils::bytearray_hash;
use starknet::ContractAddress;

/// Interface for a world's resource descriptor.
#[starknet::interface]
pub trait IDescriptor<T> {
    fn selector(self: @T) -> felt252;
    fn namespace_hash(self: @T) -> felt252;
    fn name_hash(self: @T) -> felt252;
    fn namespace(self: @T) -> ByteArray;
    fn name(self: @T) -> ByteArray;
    fn tag(self: @T) -> ByteArray;
}

/// A descriptor of a resource used to verify consistency of the selector from the namespace and the
/// name.
///
/// Fields are kept internal to ensure this struct can't be initialized with arbitrary values
/// to ensure consistency.
#[derive(Copy, Drop)]
pub struct Descriptor {
    selector: felt252,
    namespace_hash: felt252,
    name_hash: felt252,
    namespace: @ByteArray,
    name: @ByteArray,
}

/// Implements the PartialEq trait for the Descriptor, which only compares
/// the selector, since it is constructed in a consistent manner from the plain names.
impl PartialEqImpl of PartialEq<Descriptor> {
    fn eq(lhs: @Descriptor, rhs: @Descriptor) -> bool {
        (*lhs).selector() == (*rhs).selector()
    }
    fn ne(lhs: @Descriptor, rhs: @Descriptor) -> bool {
        (*lhs).selector() != (*rhs).selector()
    }
}

#[generate_trait]
pub impl DescriptorImpl of DescriptorTrait {
    /// Initializes the descriptor from plain namespace and name.
    ///
    /// # Arguments
    ///
    /// * `namespace` - The namespace of the resource.
    /// * `name` - The name of the resource.
    ///
    /// # Returns
    ///
    /// * `Descriptor` - The descriptor of the resource.
    fn from_names(namespace: @ByteArray, name: @ByteArray) -> Descriptor {
        let namespace_hash = bytearray_hash(namespace);
        let name_hash = bytearray_hash(name);
        let selector = poseidon_hash_span([namespace_hash, name_hash].span());

        Descriptor { selector, namespace_hash, name_hash, namespace, name, }
    }

    /// Initializes and asserts the descriptor from a deployed contract.
    ///
    /// # Arguments
    ///
    /// * `contract_address` - The contract address of the resource.
    ///
    /// # Returns
    ///
    /// * `Descriptor` - The descriptor of the resource.
    fn from_contract_assert(contract_address: ContractAddress) -> Descriptor {
        let d = IDescriptorDispatcher { contract_address };

        let name = d.name();
        let namespace = d.namespace();
        let namespace_hash = d.namespace_hash();
        let name_hash = d.name_hash();
        let selector = d.selector();

        let descriptor = Self::from_names(@namespace, @name);
        descriptor.assert_hashes(selector, namespace_hash, name_hash);
        descriptor
    }

    /// Asserts the provided hashes to map the descriptor, which has been initialized from plain
    /// names.
    ///
    /// # Arguments
    ///
    /// * `selector` - The selector of the resource.
    /// * `namespace_hash` - The namespace hash of the resource.
    /// * `name_hash` - The name hash of the resource.
    fn assert_hashes(
        self: @Descriptor, selector: felt252, namespace_hash: felt252, name_hash: felt252
    ) {
        if selector.is_zero() {
            panic_with_byte_array(@errors::reserved_selector(selector));
        }

        if *self.selector != selector {
            panic_with_byte_array(@errors::mismatch(@"selector", *self.selector, selector));
        }

        if *self.namespace_hash != namespace_hash {
            panic_with_byte_array(
                @errors::mismatch(@"namespace_hash", *self.namespace_hash, namespace_hash)
            );
        }

        if *self.name_hash != name_hash {
            panic_with_byte_array(@errors::mismatch(@"name_hash", *self.name_hash, name_hash));
        }
    }

    /// Gets the selector.
    fn selector(self: @Descriptor) -> felt252 {
        *self.selector
    }

    /// Gets the namespace hash.
    fn namespace_hash(self: @Descriptor) -> felt252 {
        *self.namespace_hash
    }

    /// Gets the name hash.
    fn name_hash(self: @Descriptor) -> felt252 {
        *self.name_hash
    }

    /// Gets the namespace.
    fn namespace(self: @Descriptor) -> @ByteArray {
        *self.namespace
    }

    /// Gets the name.
    fn name(self: @Descriptor) -> @ByteArray {
        *self.name
    }
}

mod errors {
    pub fn mismatch(what: @ByteArray, expected: felt252, found: felt252) -> ByteArray {
        format!("Descriptor: `{}` mismatch, expected `{}` but found `{}`", what, expected, found)
    }

    pub fn reserved_selector(found: felt252) -> ByteArray {
        format!("Descriptor: selector `{}` is a reserved selector", found)
    }
}
