use dojo::world::IWorldDispatcher;
use starknet::SyscallResult;

trait Model<T> {
    fn entity(
        world: IWorldDispatcher, keys: Span<felt252>, layout: dojo::database::introspect::Layout
    ) -> T;

    /// Returns the name of the model as it was written in Cairo code.
    fn name() -> ByteArray;
    fn version() -> u8;

    /// Returns the model selector built from its name and its namespace.
    /// model selector = hash(hash(namespace_name), hash(model_name))
    fn selector() -> felt252;
    fn instance_selector(self: @T) -> felt252;

    /// Returns the namespace of the model as it was written in the `dojo::model` attribute.
    fn namespace() -> ByteArray;

    /// Returns the model namespace selector built from its namespace.
    /// namespace_selector = hash(namespace_name)
    fn namespace_selector() -> felt252;

    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
    fn layout() -> dojo::database::introspect::Layout;
    fn instance_layout(self: @T) -> dojo::database::introspect::Layout;
    fn packed_size() -> Option<usize>;
}

#[starknet::interface]
trait IModel<T> {
    fn selector(self: @T) -> felt252;
    fn name(self: @T) -> ByteArray;
    fn version(self: @T) -> u8;
    fn namespace(self: @T) -> ByteArray;
    fn namespace_selector(self: @T) -> felt252;
    fn unpacked_size(self: @T) -> Option<usize>;
    fn packed_size(self: @T) -> Option<usize>;
    fn layout(self: @T) -> dojo::database::introspect::Layout;
    fn schema(self: @T) -> dojo::database::introspect::Ty;
}

/// Deploys a model with the given [`ClassHash`] and retrieves it's name.
/// Currently, the model is expected to already be declared by `sozo`.
///
/// # Arguments
///
/// * `salt` - A salt used to uniquely deploy the model.
/// * `class_hash` - Class Hash of the model.
fn deploy_and_get_metadata(
    salt: felt252, class_hash: starknet::ClassHash
) -> SyscallResult<(starknet::ContractAddress, ByteArray, felt252, ByteArray, felt252)> {
    let (contract_address, _) = starknet::deploy_syscall(
        class_hash, salt, array![].span(), false,
    )?;
    let model = IModelDispatcher { contract_address };
    let name = model.name();
    let selector = model.selector();
    let namespace = model.namespace();
    let namespace_selector = model.namespace_selector();
    Result::Ok((contract_address, name, selector, namespace, namespace_selector))
}
