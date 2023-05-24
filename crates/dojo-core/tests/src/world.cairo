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
use dojo_core::test_utils::mock_auth_components_systems;
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
    let mut data = ArrayTrait::new();
    data.append(1337);
    let id = World::uuid();
    World::set_entity(name, QueryTrait::new_from_id(id.into()), 0, data.span());
    let stored = World::entity(name, QueryTrait::new_from_id(id.into()), 0, 1);
    assert(*stored.snapshot.at(0) == 1337, 'data not stored');
}

#[system]
mod Bar {
    use super::Foo;
    use traits::Into;
    use starknet::get_caller_address;
    use dojo_core::integer::u250;

    fn execute(a: felt252, b: u128) {
        let caller = get_caller_address();
        commands::set_entity(caller.into(), (Foo { a, b }));
    }
}

#[test]
#[available_gas(6000000)]
fn test_system() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::new();
    data.append(1337);
    data.append(1337);
    let id = world.uuid();
    world.execute('Bar'.into(), data.span());
}

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor('World'.into(), starknet::contract_address_const::<0x1337>(), );
}

#[test]
#[available_gas(9000000)]
fn test_initialize() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());
    let mut route = ArrayTrait::new();
    let target_id = 'Bar'.into();
    let role_id = 'FooWriter'.into();
    let resource_id = 'Foo'.into();
    let r = Route { target_id, role_id, resource_id,  };
    route.append(r);

    // Initialize world
    world.initialize(route);

    // Assert that the role is stored
    let role = world.entity('AuthRole'.into(), (target_id, resource_id).into(), 0, 0);
    assert(*role[0] == 'FooWriter', 'role not stored');

    // Assert that the status is stored
    let status = world.entity('AuthStatus'.into(), (role_id, resource_id).into(), 0, 0);
    assert(*status[0] == 1, 'status not stored');

    let is_authorized = world.is_authorized(
        BarSystem::TEST_CLASS_HASH.try_into().unwrap(),
        FooComponent::TEST_CLASS_HASH.try_into().unwrap()
    );
    assert(is_authorized, 'auth route not set');
}

#[test]
#[available_gas(4000000)]
#[should_panic]
fn test_initialize_not_more_than_once() {
    // Spawn empty world
    let world = spawn_empty_world();

    // Prepare init data
    let route_a = ArrayTrait::new();
    let route_b = ArrayTrait::new();

    // Initialize world
    world.initialize(route_a);

    // Reinitialize world
    world.initialize(route_b);
}

#[test]
#[available_gas(9000000)]
fn test_set_entity_authorized() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // Prepare route
    let mut route = ArrayTrait::new();
    let target_id = 'Bar'.into();
    let role_id = 'FooWriter'.into();
    let resource_id = 'Foo'.into();
    let r = Route { target_id, role_id, resource_id,  };
    route.append(r);

    // Initialize world
    world.initialize(route);

    // Call Bar system
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('Bar'.into(), data.span());

    // Assert that the data is stored
    // Caller here is the world contract via the executor
    let world_address = world.contract_address;
    let foo = world.entity('Foo'.into(), world_address.into(), 0, 0);
    assert(*foo[0] == 420, 'data not stored');
    assert(*foo[1] == 1337, 'data not stored');
}

#[test]
#[available_gas(9000000)]
fn test_set_entity_admin() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants Admin role to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append('Admin'); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    // Call Bar system
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('Bar'.into(), data.span());

    // Assert that the data is stored
    // Caller here is the world contract via the executor
    let world_address = world.contract_address;
    let foo = world.entity('Foo'.into(), world_address.into(), 0, 0);
    assert(*foo[0] == 420, 'data not stored');
    assert(*foo[1] == 1337, 'data not stored');
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_unauthorized() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Call Bar system, should panic as it's not authorized
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('Bar'.into(), data.span());
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_directly() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // Prepare init data
    let mut route = ArrayTrait::new();
    let target_id = 'Bar'.into();
    let role_id = 'FooWriter'.into();
    let resource_id = 'Foo'.into();
    let r = Route { target_id, role_id, resource_id,  };
    route.append(r);

    // Initialize world
    world.initialize(route);

    // Change Foo component directly
    let id = world.uuid();
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.set_entity('Foo'.into(), QueryTrait::new_from_id(id.into()), 0, data.span());
}

#[test]
#[available_gas(9000000)]
fn test_grant_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants FooWriter role to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append('FooWriter'); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    // Assert that the role is set
    let role = world.entity('AuthRole'.into(), 'Bar'.into(), 0, 0);
    assert(*role[0] == 'FooWriter', 'role not granted');
}

#[test]
#[available_gas(9000000)]
fn test_revoke_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants FooWriter role to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append('FooWriter'); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    // Assert that the role is set
    let role = world.entity('AuthRole'.into(), 'Bar'.into(), 0, 0);
    assert(*role[0] == 'FooWriter', 'role not granted');

    // Admin revokes role of Bar system
    let mut revoke_role_calldata: Array<felt252> = ArrayTrait::new();
    revoke_role_calldata.append('Bar'); // target_id
    world.execute('RevokeAuthRole'.into(), revoke_role_calldata.span());

    // Assert that the role is not set
    let role = world.entity('AuthRole'.into(), 'Bar'.into(), 0, 0);
    assert(*role[0] == 0, 'role not revoked');
}

#[test]
#[available_gas(9000000)]
fn test_grant_scoped_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants FooWriter role for Foo to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append('FooWriter'); // role_id
    grant_role_calldata.append('Foo'); // resource_id
    world.execute('GrantScopedAuthRole'.into(), grant_role_calldata.span());

    // Assert that the role is set
    let role = world.entity('AuthRole'.into(), ('Bar', 'Foo').into(), 0, 0);
    assert(*role[0] == 'FooWriter', 'role not granted');
}

#[test]
#[available_gas(9000000)]
fn test_revoke_scoped_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants FooWriter role for Foo to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append('FooWriter'); // role_id
    grant_role_calldata.append('Foo'); // resource_id
    world.execute('GrantScopedAuthRole'.into(), grant_role_calldata.span());

    // Assert that the role is set
    let role = world.entity('AuthRole'.into(), ('Bar', 'Foo').into(), 0, 0);
    assert(*role[0] == 'FooWriter', 'role not granted');

    // Admin revokes role of Bar system for Foo
    let mut revoke_role_calldata: Array<felt252> = ArrayTrait::new();
    revoke_role_calldata.append('Bar'); // target_id
    revoke_role_calldata.append('Foo'); // resource_id
    world.execute('RevokeScopedAuthRole'.into(), revoke_role_calldata.span());

    // Assert that the role is revoked
    let role = world.entity('AuthRole'.into(), ('Bar', 'Foo').into(), 0, 0);
    assert(*role[0] == 0, 'role not revoked');
}

#[test]
#[available_gas(9000000)]
fn test_grant_resource() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants access for FooWriter Role to Foo
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('FooWriter'); // role_id
    grant_role_calldata.append('Foo'); // resource_id
    world.execute('GrantResource'.into(), grant_role_calldata.span());

    // Assert that the access is set
    let status = world.entity('AuthStatus'.into(), ('FooWriter', 'Foo').into(), 0, 0);
    assert(*status[0] == 1, 'access not granted');
}

#[test]
#[available_gas(9000000)]
fn test_revoke_resource() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants access for FooWriter Role to Foo
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('FooWriter'); // role_id
    grant_role_calldata.append('Foo'); // resource_id
    world.execute('GrantResource'.into(), grant_role_calldata.span());

    // Assert that the access is set
    let status = world.entity('AuthStatus'.into(), ('FooWriter', 'Foo').into(), 0, 0);
    assert(*status[0] == 1, 'access not granted');

    // Admin revokes access for FooWriter Role to Foo
    let mut revoke_role_calldata: Array<felt252> = ArrayTrait::new();
    revoke_role_calldata.append('FooWriter'); // role_id
    revoke_role_calldata.append('Foo'); // resource_id
    world.execute('RevokeResource'.into(), revoke_role_calldata.span());

    // Assert that the access is revoked
    let status = world.entity('AuthStatus'.into(), ('FooWriter', 'Foo').into(), 0, 0);
    assert(*status[0] == 0, 'access not revoked');
}

fn spawn_empty_world() -> IWorldDispatcher {
    // Deploy executor contract
    let executor_constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(),
        0,
        executor_constructor_calldata.span(),
        false
    ).unwrap();

    // Deploy world contract
    let mut constructor_calldata = array::ArrayTrait::new();
    constructor_calldata.append('World');
    constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        World::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

    // Install default auth components and systems
    let (auth_components, auth_systems) = mock_auth_components_systems();
    let mut index = 0;
    loop {
        if index == auth_components.len() {
            break ();
        }
        world.register_component(*auth_components.at(index));
        index += 1;
    };
    let mut index = 0;
    loop {
        if index == auth_systems.len() {
            break ();
        }
        world.register_system(*auth_systems.at(index));
        index += 1;
    };

    // give deployer the Admin role
    let caller = get_caller_address();
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();

    grant_role_calldata.append(caller.into()); // target_id
    grant_role_calldata.append('Admin'); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    world
}
