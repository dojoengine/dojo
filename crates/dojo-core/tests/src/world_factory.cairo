use core::traits::Into;
use core::result::ResultTrait;
use array::ArrayTrait;
use clone::Clone;
use option::OptionTrait;
use traits::TryInto;
use serde::Serde;
use debug::PrintTrait;
use starknet::syscalls::deploy_syscall;
use starknet::get_caller_address;
use starknet::class_hash::ClassHash;
use starknet::class_hash::Felt252TryIntoClassHash;
use dojo_core::interfaces::IWorldFactoryDispatcher;
use dojo_core::interfaces::IWorldFactoryDispatcherTrait;
use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::interfaces::IWorldDispatcherTrait;
use dojo_core::executor::Executor;
use dojo_core::world::World;
use dojo_core::world_factory::WorldFactory;

use dojo_core::auth::components::AuthRoleComponent;
use dojo_core::auth::systems::{Route, RouteTrait, GrantAuthRoleSystem};
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
#[available_gas(4000000)]
fn test_constructor() {
    let (auth_components, auth_systems) = mock_auth_components_systems();
    WorldFactory::constructor(
        starknet::class_hash_const::<0x420>(),
        starknet::contract_address_const::<0x69>(),
        auth_components.clone(),
        auth_systems.clone()
    );
    let world_class_hash = WorldFactory::world();
    assert(world_class_hash == starknet::class_hash_const::<0x420>(), 'wrong world class hash');
    let executor_address = WorldFactory::executor();
    assert(
        executor_address == starknet::contract_address_const::<0x69>(), 'wrong executor contract'
    );
    assert(
        WorldFactory::default_auth_components().len() == auth_components.len(),
        'wrong components length'
    );
    assert(
        WorldFactory::default_auth_systems().len() == auth_systems.len(), 'wrong systems length'
    );
}

#[test]
#[available_gas(100000000)]
fn test_spawn_world() {
    // Deploy Executor
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();

    // WorldFactory constructor
    let (auth_components, auth_systems) = mock_auth_components_systems();
    WorldFactory::constructor(
        World::TEST_CLASS_HASH.try_into().unwrap(), executor_address, auth_components, auth_systems
    );

    assert(WorldFactory::executor() == executor_address, 'wrong executor address');
    assert(
        WorldFactory::world() == World::TEST_CLASS_HASH.try_into().unwrap(),
        'wrong world class hash'
    );

    // Prepare components and systems and routes
    let mut systems = array::ArrayTrait::new();
    systems.append(BarSystem::TEST_CLASS_HASH.try_into().unwrap());

    let mut components = array::ArrayTrait::new();
    components.append(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    let mut routes = array::ArrayTrait::new();
    routes.append(RouteTrait::new('Bar'.into(), 'FooWriter'.into(), 'Foo'.into(), ));

    // Spawn World from WorldFactory
    let world_address = WorldFactory::spawn('TestWorld'.into(), components, systems, routes);
    let world = IWorldDispatcher { contract_address: world_address };

    // Check Admin role is set
    let caller = get_caller_address();
    let role = world.entity('AuthRole'.into(), caller.into(), 0, 0);
    assert(*role[0] == 'Admin', 'admin role not set');

    // Check AuthRole component and GrantAuthRole system are registered
    let role_hash = world.component('AuthRole'.into());
    assert(
        role_hash == AuthRoleComponent::TEST_CLASS_HASH.try_into().unwrap(),
        'component not registered'
    );

    let grant_role_hash = world.system('GrantAuthRole'.into());
    assert(
        grant_role_hash == GrantAuthRoleSystem::TEST_CLASS_HASH.try_into().unwrap(),
        'system not registered'
    );

    // Check Foo component and Bar system are registered
    let foo_hash = world.component('Foo'.into());
    assert(
        foo_hash == FooComponent::TEST_CLASS_HASH.try_into().unwrap(), 'component not registered'
    );

    let bar_hash = world.system('Bar'.into());
    assert(bar_hash == BarSystem::TEST_CLASS_HASH.try_into().unwrap(), 'system not registered');

    // Check that the auth routes are registered
    let role = world.entity('AuthRole'.into(), ('Bar', 'Foo').into(), 0, 0);
    assert(*role[0] == 'FooWriter', 'role not set');

    let status = world.entity('AuthStatus'.into(), (*role[0], 'Foo').into(), 0, 0);
    assert(*status[0] == 1, 'role not set');
}
