//! A simple storage abstraction for the world's storage.

use core::panic_with_felt252;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo::model::{
    Model, ModelIndex, ModelDefinition, ModelMemberStorage,
    ModelValueKey, ModelValue, ModelStorage
};
use dojo::event::{Event, EventStorage};
use dojo::meta::Layout;
use dojo::utils::{
    entity_id_from_key, serialize_inline, deserialize_unwrap, find_model_field_layout
};

#[derive(Drop)]
pub struct WorldStorage {
    pub world: IWorldDispatcher,
    pub namespace: ByteArray,
    pub namespace_hash: felt252,
}

#[generate_trait]
pub impl WorldStorageInternalImpl of WorldStorageTrait {
    fn new(world: IWorldDispatcher, namespace: ByteArray) -> WorldStorage {
        let namespace_hash = dojo::utils::bytearray_hash(@namespace);

        WorldStorage { world, namespace, namespace_hash, }
    }

    fn set_namespace(ref self: WorldStorage, namespace: ByteArray) {
        self.namespace = namespace.clone();
        self.namespace_hash = dojo::utils::bytearray_hash(@namespace);
    }
}

pub impl EventStorageWorldStorageImpl<E, +Event<E>> of EventStorage<WorldStorage, E> {
    fn emit_event(ref self: WorldStorage, event: @E) {
        dojo::world::IWorldDispatcherTrait::emit_event(
            self.world,
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
            *self.world,
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
            self.world,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::<M>::keys(model)),
            Model::<M>::values(model),
            Model::<M>::layout()
        );
    }

    fn erase_model(ref self: WorldStorage, model: @M) {
        IWorldDispatcherTrait::delete_entity(
            self.world,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::<M>::keys(model)),
            Model::<M>::layout()
        );
    }

    fn erase_model_from_key<K, +Drop<K>, +Serde<K>>(ref self: WorldStorage, key: K) {
        IWorldDispatcherTrait::delete_entity(
            self.world,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(serialize_inline::<K>(@key)),
            Model::<M>::layout()
        );
    }

    fn erase_model_from_id(ref self: WorldStorage, entity_id: felt252) {
        IWorldDispatcherTrait::delete_entity(
            self.world,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            Model::<M>::layout()
        );
    }

    fn read_member<T, K, +ModelMemberStorage<WorldStorage, M, T>, +Drop<T>, +Drop<K>, +Serde<K>>(
        self: @WorldStorage, key: K, member_id: felt252
    ) -> T {
        ModelMemberStorage::<
            WorldStorage, M, T
        >::read_member_from_id(self, entity_id_from_key::<K>(@key), member_id)
    }

    fn write_member<T, K, +ModelMemberStorage<WorldStorage, M, T>, +Drop<T>, +Drop<K>, +Serde<K>>(
        ref self: WorldStorage, key: K, member_id: felt252, value: T
    ) {
        ModelMemberStorage::<
            WorldStorage, M, T
        >::write_member_from_id(ref self, entity_id_from_key::<K>(@key), member_id, value);
    }

    fn namespace_hash(self: @WorldStorage) -> felt252 {
        *self.namespace_hash
    }
}

pub impl MemberModelStorageWorldStorageImpl<
    M, T, +Model<M>, +ModelDefinition<M>, +Serde<T>, +Drop<T>
> of ModelMemberStorage<WorldStorage, M, T> {
    fn read_member_from_id(self: @WorldStorage, entity_id: felt252, member_id: felt252) -> T {
        deserialize_unwrap::<
            T
        >(
            get_serialized_member(
                *self.world,
                Model::<M>::selector(*self.namespace_hash),
                ModelDefinition::<M>::layout(),
                entity_id,
                member_id,
            )
        )
    }

    fn write_member_from_id(ref self: WorldStorage, entity_id: felt252, member_id: felt252, value: T,) {
        update_serialized_member(
            self.world,
            Model::<M>::selector(self.namespace_hash),
            ModelDefinition::<M>::layout(),
            entity_id,
            member_id,
            serialize_inline::<T>(@value)
        )
    }

    fn namespace_hash(self: @WorldStorage) -> felt252 {
        *self.namespace_hash
    }
}

impl ModelValueStorageWorldStorageImpl<V, +ModelValue<V>> of dojo::model::ModelValueStorage<WorldStorage, V> {
    fn read_model_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        self: @WorldStorage, key: K
    ) -> V {
        Self::read_model_value_from_id(self, entity_id_from_key(@key))
    }

    fn read_model_value_from_id(self: @WorldStorage, entity_id: felt252) -> V {
        let mut values = IWorldDispatcherTrait::entity(
            *self.world,
            ModelValue::<V>::selector(*self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::layout()
        );
        match ModelValue::<V>::from_values(entity_id, ref values) {
            Option::Some(entity) => entity,
            Option::None => {
                panic!(
                    "Entity: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct."
                )
            }
        }
    }

    fn write_model_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(ref self: WorldStorage, key: K, value: @V) {
        IWorldDispatcherTrait::set_entity(
            self.world,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Keys(serialize_inline::<K>(@key)),
            ModelValue::<V>::values(value),
            ModelValue::<V>::layout()
        );
    }

    fn write_model_value_from_id(ref self: WorldStorage, entity_id: felt252, value: @V) {
        IWorldDispatcherTrait::set_entity(
            self.world,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::values(value),
            ModelValue::<V>::layout()
        );
    }
}

#[cfg(target: "test")]
pub impl EventStorageTestWorldStorageImpl<E, +Event<E>> of dojo::event::EventStorageTest<WorldStorage, E> {
    fn emit_event_test(ref self: WorldStorage, event: @E) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.world.contract_address
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
pub impl ModelStorageTestWorldStorageImpl<M, +Model<M>> of dojo::model::ModelStorageTest<WorldStorage, M> {
    fn write_model_test(ref self: WorldStorage, model: @M) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.world.contract_address
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
            contract_address: self.world.contract_address
        };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::keys(model)),
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
    fn write_model_value_test(ref self: WorldStorage, entity_id: felt252, value: @V) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.world.contract_address
        };

        dojo::world::IWorldTestDispatcherTrait::set_entity_test(
            world_test,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::values(value),
            ModelValue::<V>::layout()
        );
    }

    fn erase_model_value_test(ref self: WorldStorage, entity_id: felt252) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.world.contract_address
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
