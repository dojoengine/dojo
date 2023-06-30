use serde::Serde;
use array::{ArrayTrait, SpanTrait};
use traits::{TryInto, Into};
use option::OptionTrait;

use starknet::{ClassHash, ContractAddress};
use starknet::contract_address::Felt252TryIntoContractAddress;

use dojo::database::query::Query;
use dojo::world::Context;

#[starknet::interface]
trait IWorld<T> {
    fn component(self: @T, name: felt252) -> ClassHash;
    fn register_component(ref self: T, class_hash: ClassHash);
    fn system(self: @T, name: felt252) -> ClassHash;
    fn register_system(ref self: T, class_hash: ClassHash);
    fn uuid(ref self: T) -> usize;
    fn emit_event(self: @T, keys: Span<felt252>, values: Span<felt252>);
    fn execute(ref self: T, system: felt252, calldata: Span<felt252>) -> Span<felt252>;
    fn entity(self: @T, component: felt252, query: Query, offset: u8, length: usize) -> Span<felt252>;
    fn set_entity(
        ref self: T, component: felt252, query: Query, offset: u8, value: Span<felt252>
    );
    fn entities(self: @T, component: felt252, partition: felt252) -> (Span<felt252>, Span<Span<felt252>>);
    fn set_executor(ref self: T, contract_address: ContractAddress);
    fn executor(self: @T) -> ContractAddress;
    fn delete_entity(ref self: T, component: felt252, query: Query);

    fn is_owner(self: @T, account: ContractAddress, target: felt252) -> bool;
    fn grant_owner(ref self: T, account: ContractAddress, target: felt252);
    fn revoke_owner(ref self: T, account: ContractAddress, target: felt252);

    fn is_writer(self: @T, component: felt252, system: felt252) -> bool;
    fn grant_writer(ref self: T, component: felt252, system: felt252);
    fn revoke_writer(ref self: T, component: felt252, system: felt252);
}

#[starknet::interface]
trait IExecutor<T> {
    fn execute(
        self: @T, ctx: Context, calldata: Span<felt252>
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
    fn set_executor(ref self: T, executor_address: ContractAddress);
    fn spawn(
        ref self: T,
        components: Array<ClassHash>,
        systems: Array<ClassHash>,
    ) -> ContractAddress;
    fn world(self: @T) -> ClassHash;
    fn executor(self: @T) -> ContractAddress;
}
