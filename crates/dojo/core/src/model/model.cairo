use dojo::{
    meta::{Layout, introspect::Ty, layout::compute_packed_size},
    utils::{entity_id_from_key, serialize_inline, entity_id_from_keys}
};

use super::{ModelDefinition, ModelDef, members::MemberStore};
use super::storage::{ModelStorage, MemberModelStorage, ModelStorageTest};

/// Trait `KeyParser` defines a trait for parsing keys from a given model.
pub trait KeyParser<M, K> {
    /// Parses the key from the given model.
    fn parse_key(self: @M) -> K;
}

/// Defines a trait for parsing models, providing methods to serialize keys and values.
pub trait ModelParser<M> {
    /// Serializes the keys of the model.
    fn serialize_keys(self: @M) -> Span<felt252>;
    /// Serializes the values of the model.
    fn serialize_values(self: @M) -> Span<felt252>;
}

/// The `Model` trait.
///
/// It provides a standardized way to interact with models.
pub trait Model<M> {
    /// Parses the key from the given model, where `K` is a type containing the keys of the model.
    fn key<K, +KeyParser<M, K>>(self: @M) -> K;
    /// Returns the entity id of the model.
    fn entity_id(self: @M) -> felt252;
    /// Returns the keys of the model.
    fn keys(self: @M) -> Span<felt252>;
    /// Returns the values of the model.
    fn values(self: @M) -> Span<felt252>;
    /// Constructs a model from the given keys and values.
    fn from_values(ref keys: Span<felt252>, ref values: Span<felt252>) -> Option<M>;
    /// Returns the name of the model. (TODO: internalizing the name_hash could reduce poseidon
    /// costs).
    fn name() -> ByteArray;
    /// Returns the version of the model.
    fn version() -> u8;
    /// Returns the schema of the model.
    fn schema() -> Ty;
    /// Returns the memory layout of the model.
    fn layout() -> Layout;
    /// Returns the unpacked size of the model. Only applicable for fixed size models.
    fn unpacked_size() -> Option<usize>;
    /// Returns the packed size of the model. Only applicable for fixed size models.
    fn packed_size() -> Option<usize>;
    /// Returns the instance selector of the model.
    fn instance_layout(self: @M) -> Layout;
    /// Returns the definition of the model.
    fn definition() -> ModelDef;
    /// Returns the selector of the model computed for the given namespace hash.
    fn selector(namespace_hash: felt252) -> felt252;
}

/// The `ModelStore` trait defines a set of methods for managing models through a world dispatcher.
///
/// # Type Parameters
/// - `M`: The type of the model.
/// - `S`: The type of the storage.
/// - `K`: The type of the key used to identify models.
/// - `T`: The type of the member within the model.
pub trait ModelStore<S, M> {
    /// Retrieves a model of type `M` using the provided key of type `K`.
    fn get<K, +Drop<K>, +Serde<K>>(self: @S, key: K) -> M;
    /// Sets a model of type `M`.
    fn set(ref self: S, model: @M);
    /// Deletes a model of type `M`.
    fn delete(ref self: S, model: @M);
    /// Deletes a model of type `M` using the provided key of type `K`.
    fn delete_from_key<K, +Drop<K>, +Serde<K>>(ref self: S, key: K);
    /// Retrieves a member of type `T` from a model of type `M` using the provided member id and key
    /// of type `K`.
    fn get_member<
        T, K, +MemberStore<S, M, T>, +MemberModelStorage<S, M, T>, +Drop<T>, +Drop<K>, +Serde<K>
    >(
        self: @S, key: K, member_id: felt252
    ) -> T;
    /// Updates a member of type `T` within a model of type `M` using the provided member id, key of
    /// type `K`, and new value of type `T`.
    fn update_member<
        T,
        K,
        +MemberStore<S, M, T>,
        +MemberModelStorage<S, M, T>,
        +Drop<S>,
        +Drop<T>,
        +Drop<K>,
        +Serde<K>
    >(
        ref self: S, key: K, member_id: felt252, value: T
    );
}

pub impl ModelImpl<M, +ModelParser<M>, +ModelDefinition<M>, +Serde<M>> of Model<M> {
    fn key<K, +KeyParser<M, K>>(self: @M) -> K {
        KeyParser::<M, K>::parse_key(self)
    }

    fn entity_id(self: @M) -> felt252 {
        entity_id_from_keys(Self::keys(self))
    }

    fn keys(self: @M) -> Span<felt252> {
        ModelParser::<M>::serialize_keys(self)
    }

    fn values(self: @M) -> Span<felt252> {
        ModelParser::<M>::serialize_values(self)
    }

    fn from_values(ref keys: Span<felt252>, ref values: Span<felt252>) -> Option<M> {
        let mut serialized: Array<felt252> = keys.into();
        serialized.append_span(values);
        let mut span = serialized.span();

        Serde::<M>::deserialize(ref span)
    }

    fn name() -> ByteArray {
        ModelDefinition::<M>::name()
    }

    fn selector(namespace_hash: felt252) -> felt252 {
        dojo::utils::selector_from_namespace_and_name(namespace_hash, @Self::name())
    }

    fn version() -> u8 {
        ModelDefinition::<M>::version()
    }

    fn layout() -> Layout {
        ModelDefinition::<M>::layout()
    }

    fn schema() -> Ty {
        ModelDefinition::<M>::schema()
    }

    fn unpacked_size() -> Option<usize> {
        ModelDefinition::<M>::size()
    }

    fn packed_size() -> Option<usize> {
        compute_packed_size(ModelDefinition::<M>::layout())
    }

    fn instance_layout(self: @M) -> Layout {
        ModelDefinition::<M>::layout()
    }

    fn definition() -> ModelDef {
        ModelDef {
            name: Self::name(),
            version: Self::version(),
            layout: Self::layout(),
            schema: Self::schema(),
            packed_size: Self::packed_size(),
            unpacked_size: Self::unpacked_size()
        }
    }
}

pub impl ModelStoreImpl<S, M, +ModelStorage<S, M>, +Model<M>, +Drop<M>> of ModelStore<S, M> {
    fn get<K, +Drop<K>, +Serde<K>>(self: @S, key: K) -> M {
        ModelStorage::<S, M>::get(self, key)
    }

    fn set(ref self: S, model: @M) {
        ModelStorage::<S, M>::set(ref self, model)
    }

    fn delete(ref self: S, model: @M) {
        ModelStorage::<S, M>::delete(ref self, model)
    }

    fn delete_from_key<K, +Drop<K>, +Serde<K>>(ref self: S, key: K) {
        ModelStorage::<S, M>::delete_from_key(ref self, key)
    }

    fn get_member<
        T, K, +MemberStore<S, M, T>, +MemberModelStorage<S, M, T>, +Drop<T>, +Drop<K>, +Serde<K>
    >(
        self: @S, key: K, member_id: felt252
    ) -> T {
        MemberModelStorage::<S, M, T>::get_member(self, entity_id_from_key::<K>(@key), member_id)
    }

    fn update_member<
        T,
        K,
        +MemberStore<S, M, T>,
        +MemberModelStorage<S, M, T>,
        +Drop<S>,
        +Drop<T>,
        +Drop<K>,
        +Serde<K>
    >(
        ref self: S, key: K, member_id: felt252, value: T
    ) {
        MemberModelStorage::<
            S, M, T
        >::update_member(ref self, entity_id_from_key::<K>(@key), member_id, value);
    }
}

/// The `ModelTest` trait.
///
/// It provides a standardized way to interact with models for testing purposes,
/// bypassing the permission checks.
#[cfg(target: "test")]
pub trait ModelTest<S, M> {
    fn set_test(ref self: S, model: @M);
    fn delete_test(ref self: S, model: @M);
}

/// The `ModelTestImpl` implementation for testing purposes.
#[cfg(target: "test")]
pub impl ModelTestImpl<S, M, +ModelStorageTest<S, M>, +Model<M>> of ModelTest<S, M> {
    fn set_test(ref self: S, model: @M) {
        ModelStorageTest::<S, M>::set_test(ref self, model);
    }

    fn delete_test(ref self: S, model: @M) {
        ModelStorageTest::<S, M>::delete_test(ref self, model);
    }
}
