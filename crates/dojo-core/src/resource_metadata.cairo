//! ResourceMetadata model.
//!
//! Manually expand to ensure that dojo-core
//! does not depend on dojo plugin to be built.
//!
use dojo::world::IWorldDispatcherTrait;

use dojo::model::Model;

const RESOURCE_METADATA_SELECTOR: felt252 = selector!("ResourceMetadata");

fn initial_address() -> starknet::ContractAddress {
    starknet::contract_address_const::<0>()
}

fn initial_class_hash() -> starknet::ClassHash {
    starknet::class_hash_const::<
        0x03f75587469e8101729b3b02a46150a3d99315bc9c5026d64f2e8a061e413255
    >()
}

#[derive(Drop, Serde, PartialEq, Clone)]
struct ResourceMetadata {
    // #[key]
    resource_id: felt252,
    metadata_uri: ByteArray,
}

impl ResourceMetadataModel of dojo::model::Model<ResourceMetadata> {
    fn entity(
        world: dojo::world::IWorldDispatcher,
        keys: Span<felt252>,
        layout: dojo::database::introspect::Layout
    ) -> ResourceMetadata {
        let values = world.entity(RESOURCE_METADATA_SELECTOR, keys, layout);
        let mut serialized = core::array::ArrayTrait::new();
        core::array::serialize_array_helper(keys, ref serialized);
        core::array::serialize_array_helper(values, ref serialized);
        let mut serialized = core::array::ArrayTrait::span(@serialized);
        let entity = core::serde::Serde::<ResourceMetadata>::deserialize(ref serialized);

        if core::option::OptionTrait::<ResourceMetadata>::is_none(@entity) {
            panic!(
                "Model `ResourceMetadata`: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct."
            );
        }

        core::option::OptionTrait::<ResourceMetadata>::unwrap(entity)
    }

    #[inline(always)]
    fn name() -> ByteArray {
        "ResourceMetadata"
    }

    #[inline(always)]
    fn version() -> u8 {
        1
    }

    #[inline(always)]
    fn selector() -> felt252 {
        RESOURCE_METADATA_SELECTOR
    }

    #[inline(always)]
    fn instance_selector(self: @ResourceMetadata) -> felt252 {
        Self::selector()
    }

    #[inline(always)]
    fn keys(self: @ResourceMetadata) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::array::ArrayTrait::append(ref serialized, *self.resource_id);
        core::array::ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn values(self: @ResourceMetadata) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.metadata_uri, ref serialized);
        core::array::ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn layout() -> dojo::database::introspect::Layout {
        dojo::database::introspect::Introspect::<ResourceMetadata>::layout()
    }

    #[inline(always)]
    fn instance_layout(self: @ResourceMetadata) -> dojo::database::introspect::Layout {
        Self::layout()
    }

    #[inline(always)]
    fn packed_size() -> Option<usize> {
        Option::None
    }
}

impl ResourceMetadataIntrospect<> of dojo::database::introspect::Introspect<ResourceMetadata<>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        Option::None
    }

    #[inline(always)]
    fn layout() -> dojo::database::introspect::Layout {
        dojo::database::introspect::Layout::Struct(
            array![
                dojo::database::introspect::FieldLayout {
                    selector: selector!("metadata_uri"),
                    layout: dojo::database::introspect::Layout::ByteArray
                }
            ]
                .span()
        )
    }

    #[inline(always)]
    fn ty() -> dojo::database::introspect::Ty {
        dojo::database::introspect::Ty::Struct(
            dojo::database::introspect::Struct {
                name: 'ResourceMetadata',
                attrs: array![].span(),
                children: array![
                    dojo::database::introspect::Member {
                        name: 'resource_id',
                        ty: dojo::database::introspect::Ty::Primitive('felt252'),
                        attrs: array!['key'].span()
                    },
                    dojo::database::introspect::Member {
                        name: 'metadata_uri',
                        ty: dojo::database::introspect::Ty::ByteArray,
                        attrs: array![].span()
                    }
                ]
                    .span()
            }
        )
    }
}

#[starknet::contract]
mod resource_metadata {
    use super::ResourceMetadata;
    use super::ResourceMetadataModel;
    use super::RESOURCE_METADATA_SELECTOR;

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

    #[external(v0)]
    fn unpacked_size(self: @ContractState) -> Option<usize> {
        dojo::database::introspect::Introspect::<ResourceMetadata>::size()
    }

    #[external(v0)]
    fn packed_size(self: @ContractState) -> Option<usize> {
        ResourceMetadataModel::packed_size()
    }

    #[external(v0)]
    fn layout(self: @ContractState) -> dojo::database::introspect::Layout {
        ResourceMetadataModel::layout()
    }

    #[external(v0)]
    fn schema(self: @ContractState) -> dojo::database::introspect::Ty {
        dojo::database::introspect::Introspect::<ResourceMetadata>::ty()
    }

    #[external(v0)]
    fn ensure_abi(self: @ContractState, model: ResourceMetadata) {}
}
