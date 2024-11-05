use dojo::meta::Layout;
use dojo::meta::introspect::Struct;

/// The `IDeployedResource` starknet interface.
///
/// This is the interface used by offchain components and other contracts
/// to get some info such Dojo name from a deployed resource.
#[starknet::interface]
pub trait IDeployedResource<T> {
    fn dojo_name(self: @T) -> ByteArray;
}

/// The `IStoredResource` starknet interface.
///
/// This is the interface used by offchain components and other contracts
/// to access to storage related data of a deployed resource.
#[starknet::interface]
pub trait IStoredResource<T> {
    fn layout(self: @T) -> Layout;
    fn schema(self: @T) -> Struct;
}
