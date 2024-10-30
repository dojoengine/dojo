use dojo::meta::{Layout, Ty};
use dojo::model::ModelDef;

/// The `IModel` starknet interface.
///
/// This is the interface used by offchain components and other contracts
/// to interact with deployed models.
#[starknet::interface]
pub trait IModel<T> {
    fn dojo_name(self: @T) -> ByteArray;
    fn version(self: @T) -> u8;
    fn layout(self: @T) -> Layout;
    fn schema(self: @T) -> Ty;
    fn unpacked_size(self: @T) -> Option<usize>;
    fn packed_size(self: @T) -> Option<usize>;
    fn definition(self: @T) -> ModelDef;
}
