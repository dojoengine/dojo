use dojo::{meta::{Layout}, model::{ModelDefinition},};

pub trait ModelValueKey<V, K> {}

/// Trait `ModelValueParser` defines the interface for parsing and serializing entities of type `V`.
pub trait ModelValueParser<V> {
    /// Serializes the values of the model and returns them as a `Span<felt252>`.
    fn serialize_values(self: @V) -> Span<felt252>;
}

/// The `ModelValue` trait defines a set of methods that must be implemented by any model value type
/// `V`.
pub trait ModelValue<V> {
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

pub impl ModelValueImpl<V, +Serde<V>, +ModelDefinition<V>, +ModelValueParser<V>> of ModelValue<V> {
    fn values(self: @V) -> Span<felt252> {
        ModelValueParser::<V>::serialize_values(self)
    }

    fn from_values(entity_id: felt252, ref values: Span<felt252>) -> Option<V> {
        let mut serialized: Array<felt252> = array![];
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
