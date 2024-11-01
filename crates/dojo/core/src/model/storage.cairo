use dojo::{model::{ModelPtr, model_value::ModelValueKey}};

// TODO: define the right interface for member accesses.

/// A `ModelStorage` trait that abstracts where the storage is.
///
/// Currently it's only world storage, but this will be useful when we have other
/// storage solutions (micro worlds).
pub trait ModelStorage<S, M> {
    /// Sets a model of type `M`.
    fn write_model(ref self: S, model: @M);
    /// Retrieves a model of type `M` using the provided key of type `K`.
    fn read_model<K, +Drop<K>, +Serde<K>>(self: @S, key: K) -> M;
    /// Deletes a model of type `M`.
    fn erase_model(ref self: S, model: @M);
    /// Deletes a model of type `M` using the provided entity id.
    /// The ptr is mostly used for type inferrence.
    fn erase_model_ptr(ref self: S, ptr: ModelPtr<M>);
    /// Retrieves a model of type `M` using the provided entity idref .
    fn read_member<T, +Serde<T>>(self: @S, ptr: ModelPtr<M>, field_selector: felt252) -> T;
    /// Retrieves a model of type `M` using the provided entity id.
    fn write_member<T, +Serde<T>, +Drop<T>>(
        ref self: S, ptr: ModelPtr<M>, field_selector: felt252, value: T
    );
    /// Returns the current namespace hash.
    fn namespace_hash(self: @S) -> felt252;
}

/// A `ModelValueStorage` trait that abstracts where the storage is.
pub trait ModelValueStorage<S, V> {
    /// Retrieves a model value of type `V` using the provided key of type `K`.
    fn read_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(self: @S, key: K) -> V;

    /// Retrieves a model value of type `V` using the provided entity id.
    fn read_value_from_id(self: @S, entity_id: felt252) -> V;

    /// Updates a model value of type `V`.
    fn write_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(ref self: S, key: K, value: @V);

    /// Updates a model value of type `V`.
    fn write_value_from_id(ref self: S, entity_id: felt252, value: @V);
}

/// A `ModelStorage` trait that abstracts where the storage is.
///
/// Currently it's only world storage, but this will be useful when we have other
/// storage solutions (micro worlds).
pub trait ModelStorageTest<S, M> {
    /// Sets a model of type `M`.
    fn write_model_test(ref self: S, model: @M);
    /// Deletes a model of type `M`.
    fn erase_model_test(ref self: S, model: @M);
    /// Deletes a model of type `M` using the provided entity id.
    fn erase_model_ptr_test(ref self: S, ptr: ModelPtr<M>);
}

/// A `ModelValueStorageTest` trait that abstracts where the storage is and bypass the permission
/// checks.
pub trait ModelValueStorageTest<S, V> {
    /// Updates a model value of type `V`.
    fn write_value_test<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: S, key: K, value: @V
    );
    /// Updates a model value of type `V`.
    fn write_value_from_id_test(ref self: S, entity_id: felt252, value: @V);
    /// Deletes a model value of type `V`.
    fn erase_value_from_id_test(ref self: S, entity_id: felt252);
}
