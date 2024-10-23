use dojo::meta::Layout;
use dojo::meta::introspect::Ty;
use dojo::world::IWorldDispatcher;

#[derive(Drop, Serde, Debug, PartialEq)]
pub struct EventDefinition {
    pub name: ByteArray,
    pub version: u8,
    pub layout: Layout,
    pub schema: Ty
}

pub trait Event<T> {
    fn emit(self: @T, world: IWorldDispatcher);
    fn name() -> ByteArray;
    fn version() -> u8;
    fn definition() -> EventDefinition;
    fn layout() -> Layout;
    fn schema() -> Ty;
    fn historical() -> bool;
    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
}

#[cfg(target: "test")]
pub trait EventTest<T> {
    fn emit_test(self: @T, world: IWorldDispatcher);
}
