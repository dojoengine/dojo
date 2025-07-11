use dojo::meta::Layout;
use dojo::meta::{Introspect, introspect::{Struct, Ty}};
use dojo::model::model::ModelParser;

#[derive(Drop, Serde, Debug, PartialEq)]
pub struct EventDef {
    pub name: ByteArray,
    pub layout: Layout,
    pub schema: Struct,
}

pub trait EventDefinition<E> {
    const NAME_HASH: felt252;
    fn name() -> ByteArray;
}

pub trait Event<T> {
    fn name() -> ByteArray;
    fn definition() -> EventDef;
    fn layout() -> Layout;
    fn schema() -> Struct;
    fn serialized_keys(self: @T) -> Span<felt252>;
    fn serialized_values(self: @T) -> Span<felt252>;
    /// Returns the selector of the event computed for the given namespace hash.
    fn selector(namespace_hash: felt252) -> felt252;
}

pub impl EventImpl<E, +ModelParser<E>, +EventDefinition<E>, +Serde<E>, +Introspect<E>> of Event<E> {
    fn name() -> ByteArray {
        EventDefinition::<E>::name()
    }
    fn definition() -> EventDef {
        EventDef { name: Self::name(), layout: Self::layout(), schema: Self::schema() }
    }
    fn layout() -> Layout {
        Introspect::<E>::layout()
    }
    fn schema() -> Struct {
        match Introspect::<E>::ty() {
            Ty::Struct(s) => s,
            _ => panic!("Event: invalid schema."),
        }
    }
    fn serialized_keys(self: @E) -> Span<felt252> {
        ModelParser::<E>::serialize_keys(self)
    }
    fn serialized_values(self: @E) -> Span<felt252> {
        ModelParser::<E>::serialize_values(self)
    }
    fn selector(namespace_hash: felt252) -> felt252 {
        dojo::utils::selector_from_hashes(namespace_hash, EventDefinition::<E>::NAME_HASH)
    }
}
