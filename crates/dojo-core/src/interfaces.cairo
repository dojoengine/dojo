use dojo_core::{
    database::query::Query,
    auth::systems::Route, auth::components::AuthRole, execution_context::Context
};
use starknet::{ClassHash, ContractAddress};
use serde::Serde;
use array::{ArrayTrait, SpanTrait};
use traits::{TryInto, Into};
use option::OptionTrait;
use starknet::contract_address::Felt252TryIntoContractAddress;

#[starknet::interface]
trait IWorld<T> {
    fn initialize(ref self: T, routes: Array<Route>);
    fn component(self: @T, name: felt252) -> ClassHash;
    fn register_component(ref self: T, class_hash: ClassHash);
    fn system(self: @T, name: felt252) -> ClassHash;
    fn register_system(ref self: T, class_hash: ClassHash);
    fn uuid(self: @T) -> usize;
    fn execute(ref self: T, name: felt252, execute_calldata: Span<felt252>) -> Span<felt252>;
    fn entity(self: @T, component: felt252, key: Query, offset: u8, length: usize) -> Span<felt252>;
    fn set_entity(
        ref self: T, context: Context, component: felt252, key: Query, offset: u8, value: Span<felt252>
    );
    fn entities(self: @T, component: felt252, partition: felt252) -> (Span<felt252>, Span<Span<felt252>>);
    fn set_executor(ref self: T, contract_address: ContractAddress);
    fn executor(self: @T) -> ContractAddress;
    fn is_authorized(self: @T, system: felt252, component: felt252, execution_role: AuthRole) -> bool;
    fn is_account_admin(self: @T) -> bool;
    fn is_system_for_execution(self: @T, system: felt252) -> bool;
    fn delete_entity(ref self: T, context: Context, component: felt252, query: Query);
    fn assume_role(ref self: T, role_id: felt252, systems: Array<felt252>);
    fn clear_role(ref self: T, systems: Array<felt252>);
    fn execution_role(self: @T) -> felt252;
    fn system_components(self: @T, system: felt252) -> Array<(felt252, bool)>;
}

#[starknet::interface]
trait IExecutor<T> {
    fn execute(
        self: @T, class_hash: ClassHash, execution_role: AuthRole, execute_calldata: Span<felt252>
    ) -> Span<felt252>;
}

#[starknet::interface]
trait IComponent<T> {
    fn name(self: @T) -> felt252;
    fn len(self: @T) -> usize;
}

#[starknet::interface]
trait ISystem<T> {
    fn name(self: @T) -> felt252;
    fn dependencies(self: @T) -> Array<(felt252, bool)>;
}

#[starknet::interface]
trait IWorldFactory<T> {
    fn set_world(ref self: T, class_hash: ClassHash);
    fn set_executor(ref self: T, class_hash: ClassHash);
    fn spawn(
        self: @T,
        components: Array<ClassHash>,
        systems: Array<ClassHash>,
        routes: Array<Route>
    ) -> ContractAddress;
    fn world(self: @T) -> ClassHash;
    fn executor(self: @T) -> ContractAddress;
    fn default_auth_components(self: @T) -> Array<ClassHash>;
    fn default_auth_systems(self: @T) -> Array<ClassHash>;
}
