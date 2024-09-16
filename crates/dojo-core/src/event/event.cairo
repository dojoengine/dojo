use starknet::SyscallResult;

use dojo::meta::Layout;
use dojo::meta::introspect::Ty;
use dojo::world::IWorldDispatcher;

pub trait Event<T> {
    fn name() -> ByteArray;
    fn namespace() -> ByteArray;
    fn tag() -> ByteArray;

    fn version() -> u8;

    fn selector() -> felt252;
    fn instance_selector(self: @T) -> felt252;

    fn name_hash() -> felt252;
    fn namespace_hash() -> felt252;

    fn layout() -> Layout;
    fn schema(self: @T) -> Ty;

    fn packed_size() -> Option<usize>;
    fn unpacked_size() -> Option<usize>;
}

#[starknet::interface]
pub trait IEvent<T> {
    fn name(self: @T) -> ByteArray;
    fn namespace(self: @T) -> ByteArray;
    fn tag(self: @T) -> ByteArray;

    fn version(self: @T) -> u8;

    fn selector(self: @T) -> felt252;
    fn name_hash(self: @T) -> felt252;
    fn namespace_hash(self: @T) -> felt252;

    fn packed_size(self: @T) -> Option<usize>;
    fn unpacked_size(self: @T) -> Option<usize>;

    fn layout(self: @T) -> Layout;
    fn schema(self: @T) -> Ty;
}

/// Deploys an event with the given [`ClassHash`] and retrieves it's name.
/// Currently, the event is expected to already be declared by `sozo`.
///
/// # Arguments
///
/// * `salt` - A salt used to uniquely deploy the event.
/// * `class_hash` - Class Hash of the event.
pub fn deploy_and_get_metadata(
    salt: felt252, class_hash: starknet::ClassHash
) -> SyscallResult<(starknet::ContractAddress, ByteArray, felt252, ByteArray, felt252)> {
    let (contract_address, _) = starknet::syscalls::deploy_syscall(
        class_hash, salt, [].span(), false,
    )?;
    let event = IEventDispatcher { contract_address };
    let name = event.name();
    let selector = event.selector();
    let namespace = event.namespace();
    let namespace_hash = event.namespace_hash();
    Result::Ok((contract_address, name, selector, namespace, namespace_hash))
}
