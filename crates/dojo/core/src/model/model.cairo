use dojo::{
    meta::{Layout, introspect::Struct, layout::compute_packed_size},
    utils::{entity_id_from_serialized_keys, find_model_field_layout, entity_id_from_keys},
};

use super::{ModelDefinition, ModelDef, ModelIndex};
/// Trait `KeyParser` defines a trait for parsing keys from a given model.
///
/// A pointer to a model, which can be expressed by an entity id.
#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub struct ModelPtr<M> {
    pub id: felt252,
}

/// Trait that defines a method for converting a span of model pointers to a span of model indexes.
///
/// # Type Parameters
/// - `M`: The type of the model.
///
/// # Methods
/// - `to_indexes(self: Span<ModelPtr<M>>) -> Span<ModelIndex>`:
///   Converts the span of model pointers to a span of model indexes.
pub trait ModelPtrsTrait<M> {
    fn to_indexes(self: Span<ModelPtr<M>>) -> Span<ModelIndex>;
    fn to_schemas(self: Span<ModelPtr<M>>) -> Span<ModelIndex>;
    fn to_member_indexes(self: Span<ModelPtr<M>>, field_selector: felt252) -> Span<ModelIndex>;
}

pub impl ModelPtrsImpl<M> of ModelPtrsTrait<M> {
    fn to_indexes(self: Span<ModelPtr<M>>) -> Span<ModelIndex> {
        let mut ids = ArrayTrait::<ModelIndex>::new();
        for ptr in self {
            ids.append(ModelIndex::Id(*ptr.id));
        };
        ids.span()
    }

    fn to_schemas(self: Span<ModelPtr<M>>) -> Span<ModelIndex> {
        let mut ids = ArrayTrait::<ModelIndex>::new();
        for ptr in self {
            ids.append(ModelIndex::Schema(*ptr.id));
        };
        ids.span()
    }

    fn to_member_indexes(self: Span<ModelPtr<M>>, field_selector: felt252) -> Span<ModelIndex> {
        let mut ids = ArrayTrait::<ModelIndex>::new();
        for ptr in self {
            ids.append(ModelIndex::MemberId((*ptr.id, field_selector)));
        };
        ids.span()
    }
}

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
    fn keys<K, +KeyParser<M, K>>(self: @M) -> K;
    /// Returns the entity id of the model.
    fn entity_id(self: @M) -> felt252;
    /// Returns the keys of the model.
    fn serialized_keys(self: @M) -> Span<felt252>;
    /// Returns the values of the model.
    fn serialized_values(self: @M) -> Span<felt252>;
    /// Constructs a model from the given keys and values.
    fn from_serialized(keys: Span<felt252>, values: Span<felt252>) -> Option<M>;
    /// Returns the name of the model. (TODO: internalizing the name_hash could reduce poseidon
    /// costs).
    fn name() -> ByteArray;
    /// Returns the schema of the model.
    fn schema() -> Struct;
    /// Returns the memory layout of the model.
    fn layout() -> Layout;
    /// Returns the layout of a field in the model.
    fn field_layout(field_selector: felt252) -> Option<Layout>;
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
    /// Returns the pointer to the model from the keys.
    fn ptr_from_keys<K, +Serde<K>, +Drop<K>>(keys: K) -> ModelPtr<M>;
    /// Returns the pointer to the model from the serialised keys.
    fn ptr_from_serialized_keys(keys: Span<felt252>) -> ModelPtr<M>;
    /// Returns the pointer to the model from the entity id.
    fn ptr_from_id(entity_id: felt252) -> ModelPtr<M>;
    /// Returns the pointers to models from the keys.
    fn ptrs_from_keys<K, +Serde<K>, +Drop<K>>(keys: Span<K>) -> Span<ModelPtr<M>>;
    /// Returns the pointers to models from the serialised keys.
    fn ptrs_from_serialized_keys(keys: Span<Span<felt252>>) -> Span<ModelPtr<M>>;
    /// Returns the pointers to models from the entity ids.
    fn ptrs_from_ids(entity_ids: Span<felt252>) -> Span<ModelPtr<M>>;
    /// Returns the ptr of the model.
    fn ptr(self: @M) -> ModelPtr<M>;
}

pub impl ModelImpl<M, +ModelParser<M>, +ModelDefinition<M>, +Serde<M>, +Drop<M>> of Model<M> {
    fn keys<K, +KeyParser<M, K>>(self: @M) -> K {
        KeyParser::<M, K>::parse_key(self)
    }

    fn entity_id(self: @M) -> felt252 {
        entity_id_from_serialized_keys(Self::serialized_keys(self))
    }

    fn serialized_keys(self: @M) -> Span<felt252> {
        ModelParser::<M>::serialize_keys(self)
    }

    fn serialized_values(self: @M) -> Span<felt252> {
        ModelParser::<M>::serialize_values(self)
    }

    fn from_serialized(keys: Span<felt252>, values: Span<felt252>) -> Option<M> {
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

    fn layout() -> Layout {
        ModelDefinition::<M>::layout()
    }

    fn field_layout(field_selector: felt252) -> Option<Layout> {
        find_model_field_layout(Self::layout(), field_selector)
    }

    fn schema() -> Struct {
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
            layout: Self::layout(),
            schema: Self::schema(),
            packed_size: Self::packed_size(),
            unpacked_size: Self::unpacked_size(),
        }
    }

    fn ptr_from_keys<K, +Serde<K>, +Drop<K>>(keys: K) -> ModelPtr<M> {
        ModelPtr { id: entity_id_from_keys(@keys) }
    }

    fn ptr_from_serialized_keys(keys: Span<felt252>) -> ModelPtr<M> {
        ModelPtr { id: entity_id_from_serialized_keys(keys) }
    }

    fn ptr_from_id(entity_id: felt252) -> ModelPtr<M> {
        ModelPtr::<M> { id: entity_id }
    }

    fn ptrs_from_keys<K, +Serde<K>, +Drop<K>>(keys: Span<K>) -> Span<ModelPtr<M>> {
        let mut ptrs = ArrayTrait::<ModelPtr<M>>::new();
        for key in keys {
            ptrs.append(ModelPtr { id: entity_id_from_keys(key) });
        };
        ptrs.span()
    }

    fn ptrs_from_serialized_keys(keys: Span<Span<felt252>>) -> Span<ModelPtr<M>> {
        let mut ptrs = ArrayTrait::<ModelPtr<M>>::new();
        for key in keys {
            ptrs.append(Self::ptr_from_serialized_keys(*key));
        };
        ptrs.span()
    }

    fn ptrs_from_ids(entity_ids: Span<felt252>) -> Span<ModelPtr<M>> {
        let mut ptrs = ArrayTrait::<ModelPtr<M>>::new();
        for entity_id in entity_ids {
            ptrs.append(Self::ptr_from_id(*entity_id));
        };
        ptrs.span()
    }

    fn ptr(self: @M) -> ModelPtr<M> {
        ModelPtr::<M> { id: self.entity_id() }
    }
}
