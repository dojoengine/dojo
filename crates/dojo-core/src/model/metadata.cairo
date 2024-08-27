//! ResourceMetadata model.
//!
//! Manually expand to ensure that dojo-core
//! does not depend on dojo plugin to be built.
//!
use core::array::ArrayTrait;
use core::byte_array::ByteArray;
use core::poseidon::poseidon_hash_span;
use core::serde::Serde;

use dojo::model::introspect::{Introspect, Ty, Struct, Member};
use dojo::model::{Model, ModelIndex, Layout, FieldLayout};
use dojo::utils;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

pub fn initial_address() -> starknet::ContractAddress {
    starknet::contract_address_const::<0>()
}

pub fn initial_class_hash() -> starknet::ClassHash {
    starknet::class_hash::class_hash_const::<
        0x03f75587469e8101729b3b02a46150a3d99315bc9c5026d64f2e8a061e413255
    >()
}

#[derive(Drop, Serde, PartialEq, Clone, Debug)]
pub struct ResourceMetadata {
    // #[key]
    pub resource_id: felt252,
    pub metadata_uri: ByteArray,
}

#[generate_trait]
pub impl ResourceMetadataImpl of ResourceMetadataTrait {
    fn from_values(resource_id: felt252, ref values: Span<felt252>) -> ResourceMetadata {
        let metadata_uri = Serde::<ByteArray>::deserialize(ref values);
        if metadata_uri.is_none() {
            panic!("Model `ResourceMetadata`: metadata_uri deserialization failed.");
        }

        ResourceMetadata { resource_id, metadata_uri: metadata_uri.unwrap() }
    }
}

pub impl ResourceMetadataModel of Model<ResourceMetadata> {
    fn get(world: IWorldDispatcher, keys: Span<felt252>) -> ResourceMetadata {
        if keys.len() != 1 {
            panic!("Model `ResourceMetadata`: bad keys length.");
        };

        let mut values = world.entity(Self::selector(), ModelIndex::Keys(keys), Self::layout());
        ResourceMetadataTrait::from_values(*keys.at(0), ref values)
    }

    fn set_model(self: @ResourceMetadata, world: IWorldDispatcher,) {
        IWorldDispatcherTrait::set_entity(
            world, Self::selector(), ModelIndex::Keys(self.keys()), self.values(), Self::layout()
        );
    }

    fn delete_model(self: @ResourceMetadata, world: IWorldDispatcher,) {
        world.delete_entity(Self::selector(), ModelIndex::Keys(self.keys()), Self::layout());
    }

    fn get_member(
        world: IWorldDispatcher, keys: Span<felt252>, member_id: felt252
    ) -> Span<felt252> {
        match utils::find_model_field_layout(Self::layout(), member_id) {
            Option::Some(field_layout) => {
                let entity_id = utils::entity_id_from_keys(keys);
                world
                    .entity(
                        Self::selector(), ModelIndex::MemberId((entity_id, member_id)), field_layout
                    )
            },
            Option::None => core::panic_with_felt252('bad member id')
        }
    }

    fn set_member(
        self: @ResourceMetadata, world: IWorldDispatcher, member_id: felt252, values: Span<felt252>
    ) {
        match utils::find_model_field_layout(Self::layout(), member_id) {
            Option::Some(field_layout) => {
                world
                    .set_entity(
                        Self::selector(),
                        ModelIndex::MemberId((self.entity_id(), member_id)),
                        values,
                        field_layout
                    )
            },
            Option::None => core::panic_with_felt252('bad member id')
        }
    }

    #[inline(always)]
    fn name() -> ByteArray {
        "ResourceMetadata"
    }

    fn namespace() -> ByteArray {
        "__DOJO__"
    }

    fn tag() -> ByteArray {
        "__DOJO__-ResourceMetadata"
    }

    #[inline(always)]
    fn version() -> u8 {
        1
    }

    #[inline(always)]
    fn selector() -> felt252 {
        poseidon_hash_span([Self::namespace_hash(), Self::name_hash()].span())
    }

    #[inline(always)]
    fn instance_selector(self: @ResourceMetadata) -> felt252 {
        Self::selector()
    }

    fn name_hash() -> felt252 {
        utils::bytearray_hash(@Self::name())
    }

    fn namespace_hash() -> felt252 {
        utils::bytearray_hash(@Self::namespace())
    }

    #[inline(always)]
    fn entity_id(self: @ResourceMetadata) -> felt252 {
        poseidon_hash_span(self.keys())
    }

    #[inline(always)]
    fn keys(self: @ResourceMetadata) -> Span<felt252> {
        let mut serialized = ArrayTrait::new();
        ArrayTrait::append(ref serialized, *self.resource_id);
        ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn values(self: @ResourceMetadata) -> Span<felt252> {
        let mut serialized = ArrayTrait::new();
        Serde::serialize(self.metadata_uri, ref serialized);
        ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn layout() -> Layout {
        Introspect::<ResourceMetadata>::layout()
    }

    #[inline(always)]
    fn instance_layout(self: @ResourceMetadata) -> Layout {
        Self::layout()
    }

    #[inline(always)]
    fn packed_size() -> Option<usize> {
        Option::None
    }
}

pub impl ResourceMetadataIntrospect<> of Introspect<ResourceMetadata<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }

    #[inline(always)]
    fn layout() -> Layout {
        Layout::Struct(
            [FieldLayout { selector: selector!("metadata_uri"), layout: Layout::ByteArray }].span()
        )
    }

    #[inline(always)]
    fn ty() -> Ty {
        Ty::Struct(
            Struct {
                name: 'ResourceMetadata', attrs: [].span(), children: [
                    Member {
                        name: 'resource_id', ty: Ty::Primitive('felt252'), attrs: ['key'].span()
                    },
                    Member { name: 'metadata_uri', ty: Ty::ByteArray, attrs: [].span() }
                ].span()
            }
        )
    }
}

#[starknet::contract]
pub mod resource_metadata {
    use super::ResourceMetadata;
    use super::ResourceMetadataModel;

    use dojo::model::introspect::{Introspect, Ty};
    use dojo::model::Layout;

    #[storage]
    struct Storage {}

    #[external(v0)]
    fn selector(self: @ContractState) -> felt252 {
        ResourceMetadataModel::selector()
    }

    fn name(self: @ContractState) -> ByteArray {
        ResourceMetadataModel::name()
    }

    fn version(self: @ContractState) -> u8 {
        ResourceMetadataModel::version()
    }

    fn namespace(self: @ContractState) -> ByteArray {
        ResourceMetadataModel::namespace()
    }

    #[external(v0)]
    fn unpacked_size(self: @ContractState) -> Option<usize> {
        Introspect::<ResourceMetadata>::size()
    }

    #[external(v0)]
    fn packed_size(self: @ContractState) -> Option<usize> {
        ResourceMetadataModel::packed_size()
    }

    #[external(v0)]
    fn layout(self: @ContractState) -> Layout {
        ResourceMetadataModel::layout()
    }

    #[external(v0)]
    fn schema(self: @ContractState) -> Ty {
        Introspect::<ResourceMetadata>::ty()
    }

    #[external(v0)]
    fn ensure_abi(self: @ContractState, model: ResourceMetadata) {}
}
