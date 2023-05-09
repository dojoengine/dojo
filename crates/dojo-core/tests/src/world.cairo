use array::ArrayTrait;
use array::SpanTrait;
use core::result::ResultTrait;
use traits::Into;
use traits::TryInto;
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::syscalls::deploy_syscall;

use dojo_core::integer::u250;
use dojo_core::integer::U32IntoU250;
use dojo_core::storage::query::QueryTrait;
use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::interfaces::IWorldDispatcherTrait;
use dojo_core::executor::Executor;
use dojo_core::world::World;
use dojo_core::test_utils::spawn_test_world;
use dojo_core::auth::systems::Route;
use starknet::get_caller_address;

#[derive(Component, Copy, Drop, Serde)]
struct Foo {
    a: felt252,
    b: u128,
}

#[test]
#[available_gas(2000000)]
fn test_component() {
    let name = 'Foo'.into();
    World::register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::<felt252>::new();
    data.append(1337);
    let id = World::uuid();
    World::set_entity(name, QueryTrait::new_from_id(id.into()), 0_u8, data.span());
    let stored = World::entity(name, QueryTrait::new_from_id(id.into()), 0_u8, 1_usize);
    assert(*stored.snapshot.at(0_usize) == 1337, 'data not stored');
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
fn test_system() {
    let executor_constructor_calldata = array::ArrayTrait::<felt252>::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, executor_constructor_calldata.span(), false
    ).unwrap();

    let mut constructor_calldata = array::ArrayTrait::<felt252>::new();
    constructor_calldata.append('World');
    constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        World::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::<felt252>::new();
    data.append(1337);
    data.append(1337);
    let id = world.uuid();
    world.execute('Bar'.into(), data.span());
}

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(
        'World'.into(),
        starknet::contract_address_const::<0x1337>(),
    );
}

#[test]
#[available_gas(6000000)]
fn test_initialize() {
    // Prepare world
    let mut components = ArrayTrait::<felt252>::new();
    components.append(FooComponent::TEST_CLASS_HASH);
    let mut systems = ArrayTrait::<felt252>::new();
    systems.append(BarSystem::TEST_CLASS_HASH);

    let world = spawn_test_world(components, systems);

    // Prepare init data
    let mut route = ArrayTrait::<Route>::new();
    let target_id = 'Bar'.into();
    let role_id = 'FooWriter'.into();
    let resource_id = 'Foo'.into();
    let r = Route {
        target_id,
        role_id,
        resource_id,
    };
    route.append(r);

    // Initialize world
    world.initialize(route);

    // Assert world is initialized
    let is_initialized = world.is_initialized();
    assert(is_initialized, 'world not initialized');

    // Assert that the role is stored
    let role = world.entity('Role'.into(), QueryTrait::new_from_id(target_id.into()), 0_u8, 0_usize);
    assert(*role[0] == 'FooWriter', 'role not stored');

    // Assert that the status is stored
    let status = world.entity('Status'.into(), (role_id, resource_id).into(), 0_u8, 0_usize);
    assert(*status[0] == 1, 'status not stored');
}

#[test]
#[available_gas(3000000)]
#[should_panic]
fn test_initialize_not_more_than_once() {
    // Prepare world
    let components = ArrayTrait::<felt252>::new();
    let systems = ArrayTrait::<felt252>::new();

    let world = spawn_test_world(components, systems);

    // Prepare init data
    let route_a = ArrayTrait::<Route>::new();
    let route_b = ArrayTrait::<Route>::new();

    // Initialize world
    world.initialize(route_a);

    // Assert world is initialized
    let is_initialized = world.is_initialized();
    assert(is_initialized, 'world not initialized');

    // Reinitialize world
    world.initialize(route_b);
}

#[test]
#[available_gas(6000000)]
fn test_set_entity() {
    // Prepare world
    // components
    let mut components = array::ArrayTrait::<felt252>::new();
    components.append(FooComponent::TEST_CLASS_HASH);

    // systems
    let mut systems = array::ArrayTrait::<felt252>::new();
    systems.append(BarSystem::TEST_CLASS_HASH);

    let world = spawn_test_world(components, systems);

    // // Prepare init data
    let mut route = ArrayTrait::<Route>::new();
    let target_id = 'Bar'.into();
    let role_id = 'FooWriter'.into();
    let resource_id = 'Foo'.into();
    let r = Route {
        target_id,
        role_id,
        resource_id,
    };
    route.append(r);

    // Initialize world
    world.initialize(route);

    // Call Bar system
    let mut data = ArrayTrait::<felt252>::new();
    data.append(420);
    data.append(1337);

    let foo = world.execute('Bar'.into(), data.span());
    assert(*foo[0] == 420, 'data not stored');
    assert(*foo[1] == 1337, 'data not stored');
}
