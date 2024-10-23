use dojo::{
    meta::{Layout}, model::{ModelDefinition, ModelIndex, members::{MemberStore},},
    world::{IWorldDispatcher, IWorldDispatcherTrait}, utils::entity_id_from_key,
};

use super::storage::{ModelValueStorage, MemberModelStorage, ModelValueStorageTest};

pub trait ModelValueKey<V, K> {}

/// Trait `ModelValueParser` defines the interface for parsing and serializing entities of type `V`.
pub trait ModelValueParser<V> {
    /// Parses and returns the id of the entity as a `felt252`.
    fn parse_id(self: @V) -> felt252;
    /// Serializes the values of the model and returns them as a `Span<felt252>`.
    fn serialize_values(self: @V) -> Span<felt252>;
}

/// The `ModelValue` trait defines a set of methods that must be implemented by any model value type
/// `V`.
/// This trait provides a standardized way to interact with model values, including retrieving their
/// identifiers, values, and metadata, as well as constructing entities from values.
pub trait ModelValue<V> {
    /// Returns the unique identifier of the entity, being a hash derived from the keys.
    fn id(self: @V) -> felt252;
    /// Returns a span of values associated with the entity, every field of a model
    /// that is not a key.
    fn values(self: @V) -> Span<felt252>;
    /// Constructs a model value from its identifier and values.
    fn from_values(entity_id: felt252, ref values: Span<felt252>) -> Option<V>;
    /// Returns the name of the model value type.
    fn name() -> ByteArray;
    /// Returns the version of the model value type.
    fn version() -> u8;
    /// Returns the layout of the model value type.
    fn layout() -> Layout;
    /// Returns the layout of the model value.
    fn instance_layout(self: @V) -> Layout;
    /// Returns the selector of the model value type with the given namespace hash.
    fn selector(namespace_hash: felt252) -> felt252;
}

/// Trait `ModelValueStore` provides an interface for managing model values through a world
/// dispatcher.
pub trait ModelValueStore<S, V> {
    /// Retrieves a model value based on a given key. The key in this context is a types containing
    /// all the model keys.
    fn get_model_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(self: @S, key: K) -> V;
    /// Retrieves a model value based on its id.
    fn get_model_value_from_id(self: @S, entity_id: felt252) -> V;
    /// Updates a model value in the store.
    fn update(ref self: S, entity: @V);
    /// Deletes a model value from the store.
    fn delete_model_value(ref self: S, entity: @V);
    /// Deletes a model value based on its id.
    fn delete_from_id(ref self: S, entity_id: felt252);
    /// Retrieves a member from a model value based on its id and the member's id.
    fn get_member_from_id<T, +MemberStore<S, V, T>>(
        self: @S, entity_id: felt252, member_id: felt252
    ) -> T;
    /// Updates a member of a model value based on its id and the member's id.
    fn update_member_from_id<T, +MemberStore<S, V, T>>(
        ref self: S, entity_id: felt252, member_id: felt252, value: T
    );
}

pub impl ModelValueImpl<V, +Serde<V>, +ModelDefinition<V>, +ModelValueParser<V>> of ModelValue<V> {
    fn id(self: @V) -> felt252 {
        ModelValueParser::<V>::parse_id(self)
    }

    fn values(self: @V) -> Span<felt252> {
        ModelValueParser::<V>::serialize_values(self)
    }

    fn from_values(entity_id: felt252, ref values: Span<felt252>) -> Option<V> {
        let mut serialized: Array<felt252> = array![entity_id];
        serialized.append_span(values);
        let mut span = serialized.span();
        Serde::<V>::deserialize(ref span)
    }

    fn name() -> ByteArray {
        ModelDefinition::<V>::name()
    }

    fn version() -> u8 {
        ModelDefinition::<V>::version()
    }

    fn layout() -> Layout {
        ModelDefinition::<V>::layout()
    }

    fn instance_layout(self: @V) -> Layout {
        ModelDefinition::<V>::layout()
    }

    fn selector(namespace_hash: felt252) -> felt252 {
        dojo::utils::selector_from_namespace_and_name(namespace_hash, @Self::name())
    }
}

pub impl ModelValueStoreImpl<
    S, V, +ModelValueStorage<S, V>, +ModelValue<V>, +Drop<V>
> of ModelValueStore<S, V> {
    fn get_model_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(self: @S, key: K) -> V {
        ModelValueStorage::<S, V>::get_model_value(self, key)
    }

    fn get_model_value_from_id(self: @S, entity_id: felt252) -> V {
        ModelValueStorage::<S, V>::get_model_value_from_id(self, entity_id)
    }

    fn update(ref self: S, entity: @V) {
        ModelValueStorage::<S, V>::update(ref self, entity)
    }

    fn delete_model_value(ref self: S, entity: @V) {
        ModelValueStorage::<S, V>::delete_model_value(ref self, entity)
    }

    fn delete_from_id(ref self: S, entity_id: felt252) {
        ModelValueStorage::<S, V>::delete_from_id(ref self, entity_id)
    }

    fn get_member_from_id<T, +MemberModelStorage<S, V, T>>(
        self: @S, entity_id: felt252, member_id: felt252
    ) -> T {
        MemberModelStorage::<S, V, T>::get_member(self, entity_id, member_id)
    }

    fn update_member_from_id<T, +MemberModelStorage<S, V, T>>(
        ref self: S, entity_id: felt252, member_id: felt252, value: T
    ) {
        MemberModelStorage::<S, V, T>::update_member(ref self, entity_id, member_id, value);
    }
}

/// Test implementation of the `ModelValueTest` trait to bypass permission checks.
#[cfg(target: "test")]
pub trait ModelValueTest<S, V> {
    fn update_test(ref self: S, value: @V);
    fn delete_test(ref self: S, value: @V);
}

/// Implementation of the `ModelValueTest` trait for testing purposes, bypassing permission checks.
#[cfg(target: "test")]
pub impl ModelValueTestImpl<
    S, V, +ModelValueStorageTest<S, V>, +ModelValue<V>
> of ModelValueTest<S, V> {
    fn update_test(ref self: S, value: @V) {
        ModelValueStorageTest::<S, V>::update_test(ref self, value)
    }

    fn delete_test(ref self: S, value: @V) {
        ModelValueStorageTest::<S, V>::delete_test(ref self, value)
    }
}
