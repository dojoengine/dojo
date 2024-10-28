use dojo::meta::Layout;
use dojo::meta::introspect::Ty;

#[derive(Drop, Serde, Debug, PartialEq)]
pub struct EventDefinition {
    pub name: ByteArray,
    pub version: u8,
    pub layout: Layout,
    pub schema: Ty
}

pub trait Event<T> {
    fn name() -> ByteArray;
    fn version() -> u8;
    fn definition() -> EventDefinition;
    fn layout() -> Layout;
    fn schema() -> Ty;
    fn historical() -> bool;
    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
    /// Returns the selector of the model computed for the given namespace hash.
    fn selector(namespace_hash: felt252) -> felt252;
}
