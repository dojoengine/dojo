//! ResourceMetadata model.
//!
//! Manually expand to ensure that dojo-core
//! does not depend on dojo plugin to be built.
//!
use dojo::world::IWorldDispatcherTrait;

const RESOURCE_METADATA_MODEL: felt252 = selector!("ResourceMetadata");

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
    // #[capacity(3)].
    metadata_uri: Span<felt252>,
}

impl ResourceMetadataModel of dojo::model::Model<ResourceMetadata> {
    fn entity(
        world: dojo::world::IWorldDispatcher, keys: Span<felt252>, layout: Span<u8>
    ) -> ResourceMetadata {
        let values = world.entity(RESOURCE_METADATA_MODEL, keys, layout);
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
    fn name(self: @ResourceMetadata) -> ByteArray {
        "ResourceMetadata"
    }

    #[inline(always)]
    fn version(self: @ResourceMetadata) -> u8 {
        1
    }

    #[inline(always)]
    fn selector(self: @ResourceMetadata) -> felt252 {
        RESOURCE_METADATA_MODEL
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
    fn layout(self: @ResourceMetadata) -> Span<u8> {
        let mut layout = core::array::ArrayTrait::new();
        dojo::database::introspect::Introspect::<ResourceMetadata>::layout(ref layout);
        core::array::ArrayTrait::span(@layout)
    }

    #[inline(always)]
    fn packed_size(self: @ResourceMetadata) -> usize {
        let mut layout = self.layout();
        dojo::packing::calculate_packed_size(ref layout)
    }
}

impl ResourceMetadataIntrospect<> of dojo::database::introspect::Introspect<ResourceMetadata<>> {
    #[inline(always)]
    fn size() -> usize {
        // Length of array first + capacity.
        1 + 3
    }

    #[inline(always)]
    fn layout(ref layout: Array<u8>) {
        // Len of array first.
        layout.append(251);
        // Capacity.
        layout.append(251);
        layout.append(251);
        layout.append(251);
    }

    #[inline(always)]
    fn ty() -> dojo::database::introspect::Ty {
        dojo::database::introspect::Ty::Struct(
            dojo::database::introspect::Struct {
                name: 'ResourceMetadata',
                attrs: array![].span(),
                children: array![
                    dojo::database::introspect::serialize_member(
                        @dojo::database::introspect::Member {
                            name: 'resource_id',
                            ty: dojo::database::introspect::Ty::Primitive('felt252'),
                            attrs: array!['key'].span()
                        }
                    ),
                    dojo::database::introspect::serialize_member(
                        @dojo::database::introspect::Member {
                            name: 'metadata_uri',
                            ty: dojo::database::introspect::Ty::Array(3),
                            attrs: array![].span()
                        }
                    )
                ]
                    .span()
            }
        )
    }
}

#[starknet::contract]
mod resource_metadata {
    use super::ResourceMetadata;
    use super::RESOURCE_METADATA_MODEL;

    #[storage]
    struct Storage {}

    #[external(v0)]
    fn selector(self: @ContractState) -> felt252 {
        RESOURCE_METADATA_MODEL
    }

    fn name(self: @ContractState) -> ByteArray {
        "ResourceMetadata"
    }

    fn version(self: @ContractState) -> u8 {
        1
    }

    #[external(v0)]
    fn unpacked_size(self: @ContractState) -> usize {
        dojo::database::introspect::Introspect::<ResourceMetadata>::size()
    }

    #[external(v0)]
    fn packed_size(self: @ContractState) -> usize {
        let mut layout = core::array::ArrayTrait::new();
        dojo::database::introspect::Introspect::<ResourceMetadata>::layout(ref layout);
        let mut layout_span = layout.span();
        dojo::packing::calculate_packed_size(ref layout_span)
    }

    #[external(v0)]
    fn layout(self: @ContractState) -> Span<u8> {
        let mut layout = core::array::ArrayTrait::new();
        dojo::database::introspect::Introspect::<ResourceMetadata>::layout(ref layout);
        core::array::ArrayTrait::span(@layout)
    }

    #[external(v0)]
    fn schema(self: @ContractState) -> dojo::database::introspect::Ty {
        dojo::database::introspect::Introspect::<ResourceMetadata>::ty()
    }

    #[external(v0)]
    fn ensure_abi(self: @ContractState, model: ResourceMetadata) {}
}
