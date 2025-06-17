//! A simple storage abstraction for the world's storage.

use core::panic_with_felt252;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, Resource};
use dojo::model::{
    Model, ModelIndex, ModelValueKey, ModelValue, ModelStorage, ModelPtr, ModelPtrsTrait,
};
use dojo::event::{Event, EventStorage};
use dojo::meta::{Layout, FieldLayout, Introspect};
use dojo::utils::{
    entity_id_from_keys, entity_id_from_serialized_keys, serialize_inline, find_model_field_layout,
    deserialize_unwrap,
};
use starknet::{ContractAddress, ClassHash};

#[derive(Drop, Copy)]
pub struct WorldStorage {
    pub dispatcher: IWorldDispatcher,
    pub namespace_hash: felt252,
}

fn field_layout_unwrap<M, +Model<M>>(field_selector: felt252) -> Layout {
    match Model::<M>::field_layout(field_selector) {
        Option::Some(layout) => layout,
        Option::None => panic_with_felt252('bad member id'),
    }
}

fn make_partial_struct_layout<M, +Model<M>>(field_selectors: Span<felt252>) -> Layout {
    let mut layouts: Array<FieldLayout> = array![];
    for selector in field_selectors {
        layouts
            .append(
                FieldLayout { selector: *selector, layout: field_layout_unwrap::<M>(*selector) },
            );
    };
    Layout::Struct(layouts.span())
}

#[generate_trait]
pub impl WorldStorageInternalImpl of WorldStorageTrait {
    fn new(world: IWorldDispatcher, namespace: @ByteArray) -> WorldStorage {
        let namespace_hash = dojo::utils::bytearray_hash(namespace);

        WorldStorage { dispatcher: world, namespace_hash }
    }

    fn new_from_hash(world: IWorldDispatcher, namespace_hash: felt252) -> WorldStorage {
        WorldStorage { dispatcher: world, namespace_hash }
    }

    fn set_namespace(ref self: WorldStorage, namespace: @ByteArray) {
        self.namespace_hash = dojo::utils::bytearray_hash(namespace);
    }

    fn dns(self: @WorldStorage, contract_name: @ByteArray) -> Option<(ContractAddress, ClassHash)> {
        Self::dns_from_hash(self, dojo::utils::bytearray_hash(contract_name))
    }

    fn dns_from_hash(
        self: @WorldStorage, contract_name_hash: felt252,
    ) -> Option<(ContractAddress, ClassHash)> {
        Self::dns_from_selector(
            self, dojo::utils::selector_from_hashes(*self.namespace_hash, contract_name_hash),
        )
    }

    fn dns_from_selector(
        self: @WorldStorage, selector: felt252,
    ) -> Option<(ContractAddress, ClassHash)> {
        match (*self.dispatcher).resource(selector) {
            Resource::Contract((
                contract_address, _,
            )) => {
                let class_hash = starknet::syscalls::get_class_hash_at_syscall(contract_address)
                    .expect('Failed to get class hash');
                Option::Some((contract_address, class_hash))
            },
            Resource::Library((
                class_hash, _,
            )) => { Option::Some((starknet::contract_address_const::<0>(), class_hash)) },
            _ => Option::None,
        }
    }

    fn dns_address(self: @WorldStorage, contract_name: @ByteArray) -> Option<ContractAddress> {
        match self.dns(contract_name) {
            Option::Some((address, _)) => Option::Some(address),
            Option::None => Option::None,
        }
    }

    fn dns_class_hash(self: @WorldStorage, contract_name: @ByteArray) -> Option<ClassHash> {
        match self.dns(contract_name) {
            Option::Some((_, class_hash)) => Option::Some(class_hash),
            Option::None => Option::None,
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
            Event::<E>::serialized_keys(event),
            Event::<E>::serialized_values(event),
        );
    }
}

pub impl ModelStorageWorldStorageImpl<M, +Model<M>, +Drop<M>> of ModelStorage<WorldStorage, M> {
    fn read_model<K, +Drop<K>, +Serde<K>>(self: @WorldStorage, keys: K) -> M {
        let mut keys = serialize_inline::<K>(@keys);
        let mut values = IWorldDispatcherTrait::entity(
            *self.dispatcher,
            Model::<M>::selector(*self.namespace_hash),
            ModelIndex::Id(entity_id_from_serialized_keys(keys)),
            Model::<M>::layout(),
        );
        match Model::<M>::from_serialized(keys, values) {
            Option::Some(model) => model,
            Option::None => {
                panic!(
                    "Model: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct.",
                )
            },
        }
    }

    fn read_models<K, +Drop<K>, +Serde<K>>(self: @WorldStorage, keys: Span<K>) -> Array<M> {
        let mut indexes: Array<ModelIndex> = array![];
        let mut serialized_keys: Array<Span<felt252>> = array![];
        for k in keys {
            let sk = serialize_inline::<K>(k);
            serialized_keys.append(sk);
            indexes.append(ModelIndex::Id(entity_id_from_serialized_keys(sk)));
        };

        let all_values = IWorldDispatcherTrait::entities(
            *self.dispatcher,
            Model::<M>::selector(*self.namespace_hash),
            indexes.span(),
            Model::<M>::layout(),
        );

        let mut models: Array<M> = array![];

        let (mut i, len) = (0, indexes.len());
        while i < len {
            match Model::<M>::from_serialized(*serialized_keys[i], *all_values[i]) {
                Option::Some(model) => models.append(model),
                Option::None => {
                    panic!(
                        "Model: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct.",
                    )
                },
            };

            i += 1;
        };
        models
    }

    fn write_model(ref self: WorldStorage, model: @M) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::<M>::serialized_keys(model)),
            Model::<M>::serialized_values(model),
            Model::<M>::layout(),
        );
    }

    fn write_models(ref self: WorldStorage, models: Span<@M>) {
        let mut keys: Array<ModelIndex> = array![];
        let mut values: Array<Span<felt252>> = array![];
        for m in models {
            keys.append(ModelIndex::Keys(Model::<M>::serialized_keys(*m)));
            values.append(Model::<M>::serialized_values(*m));
        };

        IWorldDispatcherTrait::set_entities(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            keys.span(),
            values.span(),
            Model::<M>::layout(),
        );
    }

    fn erase_model(ref self: WorldStorage, model: @M) {
        IWorldDispatcherTrait::delete_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Id(Model::<M>::entity_id(model)),
            Model::<M>::layout(),
        );
    }

    fn erase_models(ref self: WorldStorage, models: Span<@M>) {
        let mut ids: Array<ModelIndex> = array![];
        for m in models {
            ids.append(ModelIndex::Id(Model::<M>::entity_id(*m)));
        };

        IWorldDispatcherTrait::delete_entities(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ids.span(),
            Model::<M>::layout(),
        );
    }

    fn erase_model_ptr(ref self: WorldStorage, ptr: ModelPtr<M>) {
        IWorldDispatcherTrait::delete_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Id(ptr.id),
            Model::<M>::layout(),
        );
    }

    fn read_member<T, +Serde<T>>(
        self: @WorldStorage, ptr: ModelPtr<M>, field_selector: felt252,
    ) -> T {
        deserialize_unwrap(
            IWorldDispatcherTrait::entity(
                *self.dispatcher,
                Model::<M>::selector(*self.namespace_hash),
                ModelIndex::MemberId((ptr.id, field_selector)),
                field_layout_unwrap::<M>(field_selector),
            ),
        )
    }

    fn read_member_of_models<T, +Serde<T>, +Drop<T>>(
        self: @WorldStorage, ptrs: Span<ModelPtr<M>>, field_selector: felt252,
    ) -> Array<T> {
        let mut values: Array<T> = array![];
        for entity in IWorldDispatcherTrait::entities(
            *self.dispatcher,
            Model::<M>::selector(*self.namespace_hash),
            ptrs.to_member_indexes(field_selector),
            field_layout_unwrap::<M>(field_selector),
        ) {
            values.append(deserialize_unwrap(*entity));
        };
        values
    }

    fn write_member<T, +Serde<T>, +Drop<T>>(
        ref self: WorldStorage, ptr: ModelPtr<M>, field_selector: felt252, value: T,
    ) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::MemberId((ptr.id, field_selector)),
            serialize_inline(@value),
            field_layout_unwrap::<M>(field_selector),
        );
    }

    fn write_member_of_models<T, +Serde<T>, +Drop<T>>(
        ref self: WorldStorage, ptrs: Span<ModelPtr<M>>, field_selector: felt252, values: Span<T>,
    ) {
        let mut serialized_values = ArrayTrait::<Span<felt252>>::new();
        for value in values {
            serialized_values.append(serialize_inline(value));
        };
        IWorldDispatcherTrait::set_entities(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ptrs.to_member_indexes(field_selector),
            serialized_values.span(),
            field_layout_unwrap::<M>(field_selector),
        );
    }

    fn erase_models_ptrs(ref self: WorldStorage, ptrs: Span<ModelPtr<M>>) {
        let mut indexes: Array<ModelIndex> = array![];
        for ptr in ptrs {
            indexes.append(ModelIndex::Id(*ptr.id));
        };

        IWorldDispatcherTrait::delete_entities(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            indexes.span(),
            Model::<M>::layout(),
        );
    }

    fn read_schema<T, +Serde<T>, +Introspect<T>>(self: @WorldStorage, ptr: ModelPtr<M>) -> T {
        deserialize_unwrap(
            IWorldDispatcherTrait::entity(
                *self.dispatcher,
                Model::<M>::selector(*self.namespace_hash),
                ModelIndex::Schema(ptr.id),
                Introspect::<T>::layout(),
            ),
        )
    }

    fn read_schemas<T, +Drop<T>, +Serde<T>, +Introspect<T>>(
        self: @WorldStorage, ptrs: Span<ModelPtr<M>>,
    ) -> Array<T> {
        let mut values = ArrayTrait::<T>::new();

        for entity in IWorldDispatcherTrait::entities(
            *self.dispatcher,
            Model::<M>::selector(*self.namespace_hash),
            ptrs.to_schemas(),
            Introspect::<T>::layout(),
        ) {
            values.append(deserialize_unwrap(*entity));
        };
        values
    }

    fn write_schema<T, +Serde<T>, +Introspect<T>>(
        ref self: WorldStorage, ptr: ModelPtr<M>, schema: @T,
    ) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Schema(ptr.id),
            serialize_inline(schema),
            Introspect::<T>::layout(),
        );
    }

    fn write_schemas<T, +Serde<T>, +Introspect<T>>(
        ref self: WorldStorage, ptrs: Span<ModelPtr<M>>, schemas: Span<@T>,
    ) {
        let mut serialized_schemas = ArrayTrait::<Span<felt252>>::new();
        for schema in schemas {
            serialized_schemas.append(serialize_inline(*schema));
        };

        IWorldDispatcherTrait::set_entities(
            self.dispatcher,
            Model::<M>::selector(self.namespace_hash),
            ptrs.to_schemas(),
            serialized_schemas.span(),
            Introspect::<T>::layout(),
        );
    }

    fn namespace_hash(self: @WorldStorage) -> felt252 {
        *self.namespace_hash
    }
}

impl ModelValueStorageWorldStorageImpl<
    V, +ModelValue<V>, +Drop<V>,
> of dojo::model::ModelValueStorage<WorldStorage, V> {
    fn read_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(self: @WorldStorage, keys: K) -> V {
        Self::read_value_from_id(self, entity_id_from_keys(@keys))
    }

    fn read_value_from_id(self: @WorldStorage, entity_id: felt252) -> V {
        let mut values = IWorldDispatcherTrait::entity(
            *self.dispatcher,
            ModelValue::<V>::selector(*self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::layout(),
        );
        match ModelValue::<V>::from_serialized(values) {
            Option::Some(entity) => entity,
            Option::None => {
                panic!(
                    "Value: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct.",
                )
            },
        }
    }

    fn read_values<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        self: @WorldStorage, keys: Span<K>,
    ) -> Array<V> {
        let mut entity_ids: Array<felt252> = array![];
        for k in keys {
            entity_ids.append(entity_id_from_keys(k));
        };

        Self::read_values_from_ids(self, entity_ids.span())
    }

    fn read_values_from_ids(self: @WorldStorage, entity_ids: Span<felt252>) -> Array<V> {
        let mut indexes: Array<ModelIndex> = array![];
        for id in entity_ids {
            indexes.append(ModelIndex::Id(*id));
        };
        let mut values = array![];
        for v in IWorldDispatcherTrait::entities(
            *self.dispatcher,
            ModelValue::<V>::selector(*self.namespace_hash),
            indexes.span(),
            ModelValue::<V>::layout(),
        ) {
            let mut v = *v;
            match ModelValue::<V>::from_serialized(v) {
                Option::Some(value) => values.append(value),
                Option::None => {
                    panic!(
                        "Value: deserialization failed. Ensure the length of the keys tuple is matching the number of #[key] fields in the model struct.",
                    )
                },
            }
        };
        values
    }

    fn write_value<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: WorldStorage, keys: K, value: @V,
    ) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            ModelValue::<V>::selector(self.namespace_hash),
            // We need Id here to trigger the store update event.
            ModelIndex::Id(entity_id_from_serialized_keys(serialize_inline::<K>(@keys))),
            ModelValue::<V>::serialized_values(value),
            ModelValue::<V>::layout(),
        );
    }

    fn write_values<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: WorldStorage, keys: Span<K>, values: Span<@V>,
    ) {
        let mut ids: Array<felt252> = array![];
        for k in keys {
            ids.append(entity_id_from_keys(k));
        };

        Self::write_values_from_ids(ref self, ids.span(), values);
    }

    fn write_value_from_id(ref self: WorldStorage, entity_id: felt252, value: @V) {
        IWorldDispatcherTrait::set_entity(
            self.dispatcher,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::serialized_values(value),
            ModelValue::<V>::layout(),
        );
    }

    fn write_values_from_ids(ref self: WorldStorage, entity_ids: Span<felt252>, values: Span<@V>) {
        let mut indexes: Array<ModelIndex> = array![];
        let mut all_values: Array<Span<felt252>> = array![];
        let mut i = 0;

        loop {
            if i >= entity_ids.len() {
                break;
            }

            indexes.append(ModelIndex::Id(*entity_ids[i]));
            all_values.append(ModelValue::<V>::serialized_values(*values[i]));

            i += 1;
        };

        IWorldDispatcherTrait::set_entities(
            self.dispatcher,
            ModelValue::<V>::selector(self.namespace_hash),
            indexes.span(),
            all_values.span(),
            ModelValue::<V>::layout(),
        );
    }
}

#[cfg(target: "test")]
pub impl EventStorageTestWorldStorageImpl<
    E, +Event<E>,
> of dojo::event::EventStorageTest<WorldStorage, E> {
    fn emit_event_test(ref self: WorldStorage, event: @E) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address,
        };
        dojo::world::IWorldTestDispatcherTrait::emit_event_test(
            world_test,
            Event::<E>::selector(self.namespace_hash),
            Event::<E>::serialized_keys(event),
            Event::<E>::serialized_values(event),
        );
    }
}

/// Implementation of the `ModelStorageTest` trait for testing purposes, bypassing permission
/// checks.
#[cfg(target: "test")]
pub impl ModelStorageTestWorldStorageImpl<
    M, +Model<M>, +Drop<M>,
> of dojo::model::ModelStorageTest<WorldStorage, M> {
    fn write_model_test(ref self: WorldStorage, model: @M) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address,
        };
        dojo::world::IWorldTestDispatcherTrait::set_entity_test(
            world_test,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::serialized_keys(model)),
            Model::<M>::serialized_values(model),
            Model::<M>::layout(),
        );
    }

    fn write_models_test(ref self: WorldStorage, models: Span<@M>) {
        for m in models {
            Self::write_model_test(ref self, *m);
        }
    }

    fn erase_model_test(ref self: WorldStorage, model: @M) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address,
        };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Keys(Model::serialized_keys(model)),
            Model::<M>::layout(),
        );
    }

    fn erase_models_test(ref self: WorldStorage, models: Span<@M>) {
        for m in models {
            Self::erase_model_test(ref self, *m);
        }
    }

    fn erase_model_ptr_test(ref self: WorldStorage, ptr: ModelPtr<M>) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address,
        };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            Model::<M>::selector(self.namespace_hash),
            ModelIndex::Id(ptr.id),
            Model::<M>::layout(),
        );
    }

    fn erase_models_ptrs_test(ref self: WorldStorage, ptrs: Span<ModelPtr<M>>) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address,
        };

        for ptr in ptrs {
            dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
                world_test,
                Model::<M>::selector(self.namespace_hash),
                ModelIndex::Id(*ptr.id),
                Model::<M>::layout(),
            );
        }
    }
}

/// Implementation of the `ModelValueStorageTest` trait for testing purposes, bypassing permission
/// checks.
#[cfg(target: "test")]
pub impl ModelValueStorageTestWorldStorageImpl<
    V, +ModelValue<V>,
> of dojo::model::ModelValueStorageTest<WorldStorage, V> {
    fn write_value_test<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: WorldStorage, keys: K, value: @V,
    ) {
        Self::write_value_from_id_test(ref self, dojo::utils::entity_id_from_keys(@keys), value);
    }

    fn write_values_test<K, +Drop<K>, +Serde<K>, +ModelValueKey<V, K>>(
        ref self: WorldStorage, keys: Span<K>, values: Span<@V>,
    ) {
        let mut ids: Array<felt252> = array![];
        for k in keys {
            ids.append(entity_id_from_keys(k));
        };

        Self::write_values_from_ids_test(ref self, ids.span(), values);
    }

    fn write_value_from_id_test(ref self: WorldStorage, entity_id: felt252, value: @V) {
        let world_test = dojo::world::IWorldTestDispatcher {
            contract_address: self.dispatcher.contract_address,
        };

        dojo::world::IWorldTestDispatcherTrait::set_entity_test(
            world_test,
            ModelValue::<V>::selector(self.namespace_hash),
            ModelIndex::Id(entity_id),
            ModelValue::<V>::serialized_values(value),
            ModelValue::<V>::layout(),
        );
    }

    fn write_values_from_ids_test(
        ref self: WorldStorage, entity_ids: Span<felt252>, values: Span<@V>,
    ) {
        let mut i = 0;
        loop {
            if i >= entity_ids.len() {
                break;
            }

            Self::write_value_from_id_test(ref self, *entity_ids[i], *values[i]);

            i += 1;
        }
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
        Option::None => panic_with_felt252('bad member id'),
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
                world, model_id, ModelIndex::MemberId((entity_id, member_id)), field_layout,
            )
        },
        Option::None => panic_with_felt252('bad member id'),
    }
}
