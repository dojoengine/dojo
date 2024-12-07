/// The `IModel` starknet interface.
///
/// This is the interface used by offchain components and other contracts
/// to interact with deployed models.
#[starknet::interface]
pub trait IModel<T> {
    fn unpacked_size(self: @T) -> Option<usize>;
    fn packed_size(self: @T) -> Option<usize>;
    fn definition(self: @T) -> dojo::model::ModelDef;
}
