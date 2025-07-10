use dojo::{model::{ModelPtr, model_value::ModelValueKey}, meta::Introspect};

// TODO: define the right interface for member accesses.

/// A `ModelStorage` trait that abstracts where the storage is.
///
/// Currently it's only world storage, but this will be useful when we have other
/// storage solutions (micro worlds).
pub trait ModelStorage<S, M> {
    /// Sets a model of type `M`.
    fn write_model(ref self: S, model: @M);

    /// Sets multiple models of type `M`.
    fn write_models(ref self: S, models: Span<@M>);

    /// Retrieves a model of type `M` using the provided key of type `K`.
    fn read_model<K, +Drop<K>, +Serde<K>>(self: @S, keys: K) -> M;

    /// Retrieves multiple models of type `M` using the provided keys of type `K`.
    /// Returnes an array to ensure the user can consume the models, even if the type is not
    /// copiable.
    fn read_models<K, +Drop<K>, +Serde<K>>(self: @S, keys: Span<K>) -> Array<M>;

    /// Deletes a model of type `M`.
    fn erase_model(ref self: S, model: @M);

    /// Deletes multiple models of type `M`.
    fn erase_models(ref self: S, models: Span<@M>);

    /// Deletes a model of type `M` using the provided entity id.
    /// The ptr is mostly used for type inferrence.
    fn erase_model_ptr(ref self: S, ptr: ModelPtr<M>);

    /// Deletes a model of type `M` using the provided entity id.
    /// The ptr is mostly used for type inferrence.
    fn erase_models_ptrs(ref self: S, ptrs: Span<ModelPtr<M>>);

    /// Retrieves a model of type `M` using the provided entity id.
    fn read_member<T, +Serde<T>>(self: @S, ptr: ModelPtr<M>, field_selector: felt252) -> T;

    /// Retrieves a single member from multiple models.
    fn read_member_of_models<T, +Serde<T>, +Drop<T>>(
        self: @S, ptrs: Span<ModelPtr<M>>, field_selector: felt252,
    ) -> Array<T>;

    /// Updates a member of a model.
    fn write_member<T, +Serde<T>, +Drop<T>>(
        ref self: S, ptr: ModelPtr<M>, field_selector: felt252, value: T,
    );

    /// Updates a member of multiple models.
    fn write_member_of_models<T, +Serde<T>, +Drop<T>>(
        ref self: S, ptrs: Span<ModelPtr<M>>, field_selector: felt252, values: Span<T>,
    );

    /// Retrieves a subset of members in a model, matching a defined schema <T>.
    fn read_schema<T, +Serde<T>, +Introspect<T>>(self: @S, ptr: ModelPtr<M>) -> T;

    /// Retrieves part of multiple models, matching a schema.
    fn read_schemas<T, +Drop<T>, +Serde<T>, +Introspect<T>>(
        self: @S, ptrs: Span<ModelPtr<M>>,
    ) -> Array<T>;

    /// Writes a subset of members in a model, matching a defined schema <T>.
    fn write_schema<T, +Serde<T>, +Introspect<T>>(ref self: S, ptr: ModelPtr<M>, schema: @T);

    /// Writes part of multiple models, matching a schema.
    fn write_schemas<T, +Serde<T>, +Introspect<T>>(
        ref self: S, ptrs: Span<ModelPtr<M>>, schemas: Span<@T>,
    );

    /// Returns the current namespace hash.
    fn namespace_hash(self: @S) -> felt252;
}

/// A `ModelValueStorage` trait that abstracts where the storage is.
pub trait ModelValueStorage<S, V> {
    /// Retrieves a model value of type `V` using the provided key of type `K`.
    fn read_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(self: @S, keys: K) -> V;

    /// Retrieves multiple model values of type `V` using the provided keys of type `K`.
    fn read_values<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        self: @S, keys: Span<K>,
    ) -> Array<V>;

    /// Retrieves a model value of type `V` using the provided entity id.
    fn read_value_from_id(self: @S, entity_id: felt252) -> V;

    /// Retrieves multiple model values of type `V` using the provided entity ids.
    fn read_values_from_ids(self: @S, entity_ids: Span<felt252>) -> Array<V>;

    /// Updates a model value of type `V`.
    fn write_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(ref self: S, keys: K, value: @V);

    /// Updates multiple model values of type `V`.
    fn write_values<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: S, keys: Span<K>, values: Span<@V>,
    );

    /// Updates a model value of type `V`.
    fn write_value_from_id(ref self: S, entity_id: felt252, value: @V);

    /// Updates multiple model values of type `V`.
    fn write_values_from_ids(ref self: S, entity_ids: Span<felt252>, values: Span<@V>);
}

/// A `ModelStorage` trait that abstracts where the storage is.
///
/// Currently it's only world storage, but this will be useful when we have other
/// storage solutions (micro worlds).
pub trait ModelStorageTest<S, M> {
    /// Sets a model of type `M`.
    fn write_model_test(ref self: S, model: @M);
    /// Sets multiple models of type `M`.
    fn write_models_test(ref self: S, models: Span<@M>);
    /// Deletes a model of type `M`.
    fn erase_model_test(ref self: S, model: @M);
    /// Deletes multiple models of type `M`.
    fn erase_models_test(ref self: S, models: Span<@M>);
    /// Deletes a model of type `M` using the provided entity id.
    fn erase_model_ptr_test(ref self: S, ptr: ModelPtr<M>);
    /// Deletes multiple models of type `M` using the provided entity ids.
    fn erase_models_ptrs_test(ref self: S, ptrs: Span<ModelPtr<M>>);
}

/// A `ModelValueStorageTest` trait that abstracts where the storage is and bypass the permission
/// checks.
pub trait ModelValueStorageTest<S, V> {
    /// Updates a model value of type `V`.
    fn write_value_test<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: S, keys: K, value: @V,
    );
    /// Updates multiple model values of type `V`.
    fn write_values_test<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: S, keys: Span<K>, values: Span<@V>,
    );
    /// Updates a model value of type `V`.
    fn write_value_from_id_test(ref self: S, entity_id: felt252, value: @V);
    /// Updates multiple model values of type `V`.
    fn write_values_from_ids_test(ref self: S, entity_ids: Span<felt252>, values: Span<@V>);
}
