use dojo::meta::Layout;
use dojo::meta::introspect::Ty;

use super::EventDefinition;

#[starknet::interface]
pub trait IEvent<T> {
    fn dojo_name(self: @T) -> ByteArray;
    fn version(self: @T) -> u8;
    fn definition(self: @T) -> EventDefinition;
    fn layout(self: @T) -> Layout;
    fn schema(self: @T) -> Ty;
}
