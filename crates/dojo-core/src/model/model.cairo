use starknet::SyscallResult;

use dojo::model::Layout;
use dojo::model::introspect::Ty;
use dojo::world::IWorldDispatcher;

#[derive(Drop, Clone, Serde, Debug)]
pub struct ModelDefinition {
    pub selector: felt252,
    pub namespace: ByteArray,
    pub name: ByteArray,
    pub version: u8,
    pub layout: Layout,
    pub ty: Ty,
    pub packed_size: Option<u32>,
    pub unpacked_size: Option<u32>
}

#[derive(Drop, Serde, Debug)]
pub struct ModelInfo {
    pub namespace: ByteArray,
    pub name: ByteArray,
    pub version: u8,
}

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub enum ModelIndex {
    Keys: Span<felt252>,
    Id: felt252,
    // (entity_id, member_id)
    MemberId: (felt252, felt252)
}

/// Trait that is implemented at Cairo level for each struct that is a model.
pub trait ModelEntity<T> {
    fn id(self: @T) -> felt252;
    fn values(self: @T) -> Span<felt252>;
    fn from_values(entity_id: felt252, ref values: Span<felt252>) -> T;
    // Get is always used with the trait path, which results in no ambiguity for the compiler.
    fn get(world: IWorldDispatcher, entity_id: felt252) -> T;
    // Update and delete can be used directly on the entity, which results in ambiguity.
    // Therefore, they are implemented with the `update_entity` and `delete_entity` names.
    fn update_entity(self: @T, world: IWorldDispatcher);
    fn delete_entity(self: @T, world: IWorldDispatcher);
    fn get_member(
        world: IWorldDispatcher, entity_id: felt252, member_id: felt252,
    ) -> Span<felt252>;
    fn set_member(self: @T, world: IWorldDispatcher, member_id: felt252, values: Span<felt252>);
}

pub trait Model<T> {
    // Get is always used with the trait path, which results in no ambiguity for the compiler.
    fn get(world: IWorldDispatcher, keys: Span<felt252>) -> T;
    // Note: `get` is implemented with a generated trait because it takes
    // the list of model keys as separated parameters.

    // Set and delete can be used directly on the entity, which results in ambiguity.
    // Therefore, they are implemented with the `set_model` and `delete_model` names.
    fn set_model(self: @T, world: IWorldDispatcher);
    fn delete_model(self: @T, world: IWorldDispatcher);

    fn get_member(
        world: IWorldDispatcher, keys: Span<felt252>, member_id: felt252,
    ) -> Span<felt252>;

    fn set_member(self: @T, world: IWorldDispatcher, member_id: felt252, values: Span<felt252>,);

    /// Returns the name of the model as it was written in Cairo code.
    fn name() -> ByteArray;

    /// Returns the namespace of the model as it was written in the `dojo::model` attribute.
    fn namespace() -> ByteArray;

    // Returns the model tag which combines the namespace and the name.
    fn tag() -> ByteArray;

    // Returns the full model definition as stored in the world contract.
    fn definition() -> ModelDefinition;

    fn version() -> u8;

    /// Returns the model selector built from its name and its namespace.
    /// model selector = hash(namespace_hash, model_hash)
    fn selector() -> felt252;
    fn instance_selector(self: @T) -> felt252;

    fn name_hash() -> felt252;
    fn namespace_hash() -> felt252;

    fn entity_id(self: @T) -> felt252;
    fn keys(self: @T) -> Span<felt252>;
    fn values(self: @T) -> Span<felt252>;
    fn layout() -> Layout;
    fn ty() -> Ty;
    fn instance_layout(self: @T) -> Layout;
    fn packed_size() -> Option<usize>;
    fn unpacked_size() -> Option<usize>;
}

#[cfg(target: "test")]
pub trait ModelTest<T> {
    fn set_test(self: @T, world: IWorldDispatcher);
    fn delete_test(self: @T, world: IWorldDispatcher);
}

#[cfg(target: "test")]
pub trait ModelEntityTest<T> {
    fn update_test(self: @T, world: IWorldDispatcher);
    fn delete_test(self: @T, world: IWorldDispatcher);
}
