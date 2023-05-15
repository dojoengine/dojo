use core::traits::Into;
use core::result::ResultTrait;
use array::ArrayTrait;
use option::OptionTrait;
use traits::TryInto;

use starknet::syscalls::deploy_syscall;
use starknet::get_caller_address;
use starknet::class_hash::ClassHash;
use starknet::class_hash::Felt252TryIntoClassHash;
use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::interfaces::IWorldDispatcherTrait;
use dojo_core::executor::Executor;
use dojo_core::world::World;
use dojo_core::world_factory::WorldFactory;

use dojo_core::auth::components::RoleComponent;
use dojo_core::auth::systems::GrantRoleSystem;
use dojo_core::test_utils::mock_auth_components_systems;

#[derive(Component, Copy, Drop, Serde)]
struct Foo {
    a: felt252,
    b: u128,
}

#[system]
mod Bar {
    use super::Foo;

    fn execute(foo: Foo) -> Foo {
        foo
    }
}

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    WorldFactory::constructor(
        starknet::class_hash_const::<0x420>(), starknet::contract_address_const::<0x69>()
    );
    let world_class_hash = WorldFactory::world();
    assert(world_class_hash == starknet::class_hash_const::<0x420>(), 'wrong world class hash');
    let executor_address = WorldFactory::executor();
    assert(
        executor_address == starknet::contract_address_const::<0x69>(), 'wrong executor contract'
    );
}

#[test]
#[available_gas(40000000)]
fn test_spawn_world() {
    let constructor_calldata = array::ArrayTrait::<felt252>::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();

    WorldFactory::constructor(World::TEST_CLASS_HASH.try_into().unwrap(), executor_address);
    assert(WorldFactory::executor() == executor_address, 'wrong executor address');
    assert(
        WorldFactory::world() == World::TEST_CLASS_HASH.try_into().unwrap(),
        'wrong world class hash'
    );

    // Get test auth components and systems class hashes
    let (auth_components, auth_systems) = mock_auth_components_systems();

    let mut systems = array::ArrayTrait::<ClassHash>::new();
    systems.append(BarSystem::TEST_CLASS_HASH.try_into().unwrap());

    let mut components = array::ArrayTrait::<ClassHash>::new();
    components.append(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // TODO: Add set_account_contract_address when it's available to use to set custom deployer
    let world_address = WorldFactory::spawn('TestWorld'.into(), components, systems, auth_components, auth_systems);

    // Check Admin role is set
    let caller = get_caller_address();
    let role = IWorldDispatcher { contract_address: world_address }.entity('Role'.into(), caller.into(), 0_u8, 0_usize);
    assert(*role[0] == 'Admin', 'admin role not set');

    // Check Role component and GrantRole system are registered
    let role_hash = IWorldDispatcher { contract_address: world_address }.component('Role'.into());
    assert(
        role_hash == RoleComponent::TEST_CLASS_HASH.try_into().unwrap(), 'component not registered'
    );

    let grant_role_hash = IWorldDispatcher { contract_address: world_address }.system('GrantRole'.into());
    assert(grant_role_hash == GrantRoleSystem::TEST_CLASS_HASH.try_into().unwrap(), 'system not registered');

    // Check Foo component and Bar system are registered
    let foo_hash = IWorldDispatcher { contract_address: world_address }.component('Foo'.into());
    assert(
        foo_hash == FooComponent::TEST_CLASS_HASH.try_into().unwrap(), 'component not registered'
    );

    let bar_hash = IWorldDispatcher { contract_address: world_address }.system('Bar'.into());
    assert(bar_hash == BarSystem::TEST_CLASS_HASH.try_into().unwrap(), 'system not registered');
}
