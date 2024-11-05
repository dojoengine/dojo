//! A simple storage abstraction for the world's storage.

use core::panic_with_felt252;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, Resource};
use dojo::model::{Model, ModelIndex, ModelValueKey, ModelValue, ModelStorage, ModelPtr};
use dojo::event::{Event, EventStorage};
use dojo::meta::Layout;
use dojo::utils::{
    entity_id_from_key, entity_id_from_keys, serialize_inline, find_model_field_layout
};
use starknet::{ContractAddress, ClassHash};

#[derive(Drop, Copy)]
pub struct WorldStorage {
    pub dispatcher: IWorldDispatcher,
    pub namespace_hash: felt252,
}

#[generate_trait]
pub impl WorldStorageInternalImpl of WorldStorageTrait {
    fn new(world: IWorldDispatcher, namespace: @ByteArray) -> WorldStorage {
        let namespace_hash = dojo::utils::bytearray_hash(namespace);

        WorldStorage { dispatcher: world, namespace_hash }
    }

    fn set_namespace(ref self: WorldStorage, namespace: @ByteArray) {
        self.namespace_hash = dojo::utils::bytearray_hash(namespace);
    }

    fn dns(self: @WorldStorage, contract_name: @ByteArray) -> Option<(ContractAddress, ClassHash)> {
        match (*self.dispatcher)
            .resource(
                dojo::utils::selector_from_namespace_and_name(*self.namespace_hash, contract_name)
            ) {
            Resource::Contract((
                contract_address, class_hash
            )) => Option::Some((contract_address, class_hash.try_into().unwrap())),
            _ => Option::None
        }
    }

    fn resource_selector(self: @WorldStorage, name: @ByteArray) -> felt252 {
        dojo::utils::selector_from_namespace_and_name(*self.namespace_hash, name)
    }
}

pub impl EventStorageWorldStorageImpl<E, +Event<E>> of EventStorage<WorldStorage, E> {
    fn emit_event(ref self: WorldStorage, event: @E) {
        dojo::world::IWorldDispatcherTrait::emit_event(
            self.dispatcher,
            Event::<E>::selector(self.namespace_hash),
            Event::<E>::keys(event),
            Event::<E>::values(event),
            Event::<E>::historical()
        );
    }
}

pub impl ModelStorageWorldStorageImpl<M, +Model<M>, +Drop<M>> of ModelStorage<WorldStorage, M> {
    fn read_model<K, +Drop<K>, +Serde<K>>(self: @WorldStorage, key: K) -> M {
        let mut keys = serialize_inline::<K>(@key);
        let mut values = IWorldDispatcherTrait::entity(
            *self.dispatcher,
            Model::<M>::selector(*self.namespace_hash),
            ModelIndex::Keys(keys),
            Model::<M>::layout()
        );
        match Model::<M>::from_values(ref keys, ref values) {
            Option::Some(model) => model,
            Option::None => {
                panic!(
                    "Model: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct."
                )
            }
        }
    }

    fn write_model(ref self: WorldStorage, model: @M) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::<M>::keys(model)),
            Model::<M>::values(model),
            Model::<M>::layout()
        );
    }

    fn erase_model(ref self: WorldStorage, model: @M) {
        IWorldDispatcherTrait::delete_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::<M>::keys(model)),
            Model::<M>::layout()
        );
    }

    fn erase_model_ptr(ref self: WorldStorage, ptr: ModelPtr<M>) {
        let entity_id = match ptr {
            ModelPtr::Id(id) => id,
            ModelPtr::Keys(keys) => entity_id_from_keys(keys),
        };

        IWorldDispatcherTrait::delete_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            Model::<M>::layout()
        );
    }

    fn namespace_hash(self: @WorldStorage) -> felt252 {
        *self.namespace_hash
    }
}

impl ModelValueStorageWorldStorageImpl<
    V, +ModelValue<V>
> of dojo::model::ModelValueStorage<WorldStorage, V> {
    fn read_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(self: @WorldStorage, key: K) -> V {
        Self::read_value_from_id(self, entity_id_from_key(@key))
    }

    fn read_value_from_id(self: @WorldStorage, entity_id: felt252) -> V {
        let mut values = IWorldDispatcherTrait::entity(
            *self.dispatcher,
            ModelValue::<V>::selector(*self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::layout()
        );
        match ModelValue::<V>::from_values(entity_id, ref values) {
            Option::Some(entity) => entity,
            Option::None => {
                panic!(
                    "Value: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct."
                )
            }
        }
    }

    fn write_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: WorldStorage, key: K, value: @V
    ) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            ModelValue::<V>::selector(self.namespace_hash),
            // We need Id here to trigger the store update event.
            ModelIndex::Id(entity_id_from_keys(serialize_inline::<K>(@key))),
            ModelValue::<V>::values(value),
            ModelValue::<V>::layout()
        );
    }

    fn write_value_from_id(ref self: WorldStorage, entity_id: felt252, value: @V) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::values(value),
            ModelValue::<V>::layout()
        );
    }
}

#[cfg(target: "test")]
pub impl EventStorageTestWorldStorageImpl<
    E, +Event<E>
> of dojo::event::EventStorageTest<WorldStorage, E> {
    fn emit_event_test(ref self: WorldStorage, event: @E) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address
        };
        dojo::world::IWorldTestDispatcherTrait::emit_event_test(
            world_test,
            Event::<E>::selector(self.namespace_hash),
            Event::<E>::keys(event),
            Event::<E>::values(event),
            Event::<E>::historical()
        );
    }
}

/// Implementation of the `ModelStorageTest` trait for testing purposes, bypassing permission
/// checks.
#[cfg(target: "test")]
pub impl ModelStorageTestWorldStorageImpl<
    M, +Model<M>
> of dojo::model::ModelStorageTest<WorldStorage, M> {
    fn write_model_test(ref self: WorldStorage, model: @M) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address
        };
        dojo::world::IWorldTestDispatcherTrait::set_entity_test(
            world_test,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::keys(model)),
            Model::<M>::values(model),
            Model::<M>::layout()
        );
    }

    fn erase_model_test(ref self: WorldStorage, model: @M) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address
        };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::keys(model)),
            Model::<M>::layout()
        );
    }

    fn erase_model_ptr_test(ref self: WorldStorage, ptr: ModelPtr<M>) {
        let entity_id = match ptr {
            ModelPtr::Id(id) => id,
            ModelPtr::Keys(keys) => entity_id_from_keys(keys),
        };

        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address
        };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            Model::<M>::layout()
        );
    }
}

/// Implementation of the `ModelValueStorageTest` trait for testing purposes, bypassing permission
/// checks.
#[cfg(target: "test")]
pub impl ModelValueStorageTestWorldStorageImpl<
    V, +ModelValue<V>
> of dojo::model::ModelValueStorageTest<WorldStorage, V> {
    fn write_value_test<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: WorldStorage, key: K, value: @V
    ) {
        let keys = serialize_inline::<K>(@key);
        Self::write_value_from_id_test(ref self, dojo::utils::entity_id_from_keys(keys), value);
    }

    fn write_value_from_id_test(ref self: WorldStorage, entity_id: felt252, value: @V) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address
        };

        dojo::world::IWorldTestDispatcherTrait::set_entity_test(
            world_test,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::values(value),
            ModelValue::<V>::layout()
        );
    }

    fn erase_value_from_id_test(ref self: WorldStorage, entity_id: felt252) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address
        };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::layout()
        );
    }
}

/// Updates a serialized member of a model.
fn update_serialized_member(
    world: IWorldDispatcher,
    model_id: felt252,
    layout: Layout,
    entity_id: felt252,
    member_id: felt252,
    values: Span<felt252>,
) {
    match find_model_field_layout(layout, member_id) {
        Option::Some(field_layout) => {
            IWorldDispatcherTrait::set_entity(
                world, model_id, ModelIndex::MemberId((entity_id, member_id)), values, field_layout,
            )
        },
        Option::None => panic_with_felt252('bad member id')
    }
}

/// Retrieves a serialized member of a model.
fn get_serialized_member(
    world: IWorldDispatcher,
    model_id: felt252,
    layout: Layout,
    entity_id: felt252,
    member_id: felt252,
) -> Span<felt252> {
    match find_model_field_layout(layout, member_id) {
        Option::Some(field_layout) => {
            IWorldDispatcherTrait::entity(
                world, model_id, ModelIndex::MemberId((entity_id, member_id)), field_layout
            )
        },
        Option::None => panic_with_felt252('bad member id')
    }
}
