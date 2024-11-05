use dojo::meta::Layout;
use dojo::meta::introspect::Struct;

#[derive(Drop, Serde, Debug, PartialEq)]
pub struct EventDefinition {
    pub name: ByteArray,
    pub layout: Layout,
    pub schema: Struct
}

pub trait Event<T> {
    fn name() -> ByteArray;
    fn definition() -> EventDefinition;
    fn layout() -> Layout;
    fn schema() -> Struct;
    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
    /// Returns the selector of the event computed for the given namespace hash.
    fn selector(namespace_hash: felt252) -> felt252;
}
