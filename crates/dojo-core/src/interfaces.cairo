use dojo_core::{
    serde::SpanSerde, storage::key::Key,
    auth::systems::Route, auth::components::AuthRole, execution_context::Context
};
use starknet::{ClassHash, ContractAddress};
use serde::Serde;
use array::{ArrayTrait, SpanTrait};
use traits::{TryInto, Into};
use option::OptionTrait;
use starknet::contract_address::Felt252TryIntoContractAddress;


#[abi]
trait IWorld {
    fn initialize(routes: Array<Route>);
    fn component(name: felt252) -> ClassHash;
    fn register_component(class_hash: ClassHash);
    fn system(name: felt252) -> ClassHash;
    fn register_system(class_hash: ClassHash);
    fn uuid() -> usize;
    fn execute(name: felt252, execute_calldata: Span<felt252>) -> Span<felt252>;
    fn entity(component: felt252, key: Key, offset: u8, length: usize) -> Span<felt252>;
    fn set_entity(
        context: Context, component: felt252, key: Key, offset: u8, value: Span<felt252>
    );
    fn entities(component: felt252, index: u256) -> (Span<felt252>, Span<Span<felt252>>);
    fn set_executor(contract_address: ContractAddress);
    fn is_authorized(system: felt252, component: felt252, execution_role: AuthRole) -> bool;
    fn is_account_admin() -> bool;
    fn is_system_for_execution(system: felt252) -> bool;
    fn delete_entity(context: Context, component: felt252, key: Key);
    fn assume_role(role_id: felt252, systems: Array<felt252>);
    fn clear_role(systems: Array<felt252>);
    fn execution_role() -> felt252;
    fn system_components(system: felt252) -> Array<(felt252, bool)>;
}

// TODO: Remove once Serde is derivable for dispatchers
impl IWorldDispatcherSerde of Serde<IWorldDispatcher> {
    fn serialize(self: @IWorldDispatcher, ref output: Array<felt252>) {
        output.append((*self.contract_address).into());
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<IWorldDispatcher> {
        let contract_address: felt252 = *serialized.pop_front()?;
        let contract_address: ContractAddress = contract_address.try_into().unwrap();
        Option::Some(IWorldDispatcher { contract_address })
    }
}

#[abi]
trait IExecutor {
    fn execute(
        class_hash: ClassHash, execution_role: AuthRole, execute_calldata: Span<felt252>
    ) -> Span<felt252>;
}

#[abi]
trait IComponent {
    fn name() -> felt252;
    fn len() -> usize;
}

#[abi]
trait ISystem {
    fn name() -> felt252;
    fn dependencies() -> Array<(felt252, bool)>;
}

#[abi]
trait IWorldFactory {
    fn set_world(class_hash: ClassHash);
    fn set_executor(class_hash: ClassHash);
    fn spawn(
        name: felt252,
        components: Array<ClassHash>,
        systems: Array<ClassHash>,
        routes: Array<Route>
    );
    fn world_class_hash() -> ClassHash;
    fn executor_address() -> ContractAddress;
    fn default_auth_components() -> Array<ClassHash>;
    fn default_auth_systems() -> Array<ClassHash>;
}
