use array::ArrayTrait;
use array::SpanTrait;
use clone::Clone;
use core::result::ResultTrait;
use traits::Into;
use traits::TryInto;
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::contract_address_const;
use starknet::get_caller_address;
use starknet::syscalls::deploy_syscall;

use dojo::database::query::QueryTrait;
use dojo::interfaces::IWorldDispatcher;
use dojo::interfaces::IWorldDispatcherTrait;
use dojo::executor::Executor;
use dojo::execution_context::Context;
use dojo::auth::components::AuthRole;
use dojo::world::World;
use dojo::world::LibraryCall;
use dojo::auth::systems::Route;

use dojo::test_utils::mock_auth_components_systems;

// Components and Systems

#[derive(Component, Copy, Drop, Serde)]
struct Foo {
    a: felt252,
    b: u128,
}

#[derive(Component, Copy, Drop, Serde)]
struct Fizz {
    a: felt252
}

#[system]
mod Bar {
    use super::Foo;
    use traits::Into;
    use starknet::get_caller_address;

    fn execute(ctx: Context, a: felt252, b: u128) {
        let caller = get_caller_address();
        set !(ctx, caller.into(), (Foo { a, b }));
    }
}

#[system]
mod Buzz {
    use super::{Foo, Fizz};
    use traits::Into;
    use starknet::get_caller_address;

    fn execute(ctx: Context, a: felt252, b: u128) {
        let caller = get_caller_address();
        set !(ctx, caller.into(), (Foo { a, b }));
        let fizz = try_get !(ctx, ctx.caller_account.into(), Fizz);
    }
}

// Tests

fn deploy_world() -> IWorldDispatcher {
    let mut calldata: Array<felt252> = array::ArrayTrait::new();
    calldata.append(starknet::contract_address_const::<0x0>().into());
    let (world_address, _) = deploy_syscall(
        World::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
    )
        .unwrap();

    IWorldDispatcher { contract_address: world_address }
}

#[test]
#[available_gas(2000000)]
fn test_component() {
    let name = 'Foo'.into();
    let world = deploy_world();

    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::new();
    data.append(1337);
    let id = world.uuid();
    let ctx = Context {
        world,
        caller_account: contract_address_const::<0x1337>(),
        caller_system: 'Bar'.into(),
        execution_role: AuthRole {
            id: 'FooWriter'.into()
        },
    };

    world.set_entity(ctx, name, QueryTrait::new_from_id(id.into()), 0, data.span());
    let stored = world.entity(name, QueryTrait::new_from_id(id.into()), 0, 1);
    assert(*stored.snapshot.at(0) == 1337, 'data not stored');
}

#[test]
#[available_gas(2000000)]
fn test_component_with_partition() {
    let name = 'Foo'.into();
    let world = deploy_world();

    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::new();
    data.append(1337);
    let id = world.uuid();
    let ctx = Context {
        world,
        caller_account: contract_address_const::<0x1337>(),
        caller_system: 'Bar'.into(),
        execution_role: AuthRole {
            id: 'FooWriter'.into()
        },
    };

    let mut keys = ArrayTrait::new();
    keys.append(1337.into());
    world.set_entity(ctx, name, QueryTrait::new(0, 1.into(), keys.span()), 0, data.span());
    let stored = world.entity(name, QueryTrait::new(0, 1.into(), keys.span()), 0, 1);
    assert(*stored.snapshot.at(0) == 1337, 'data not stored');
}

#[test]
#[available_gas(6000000)]
fn test_system() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::new();
    data.append(1337);
    data.append(1337);
    let id = world.uuid();
    world.execute('Bar'.into(), data.span());
}

// #[test]
// #[available_gas(1000000)]
// fn test_system_components() {
//     let world = spawn_empty_world();

//     // Register components and systems
//     world.register_system(Buzz::TEST_CLASS_HASH.try_into().unwrap());
//     world.register_component(FizzComponent::TEST_CLASS_HASH.try_into().unwrap());
//     world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

//     // Get system components
//     let components = world.system_components('Buzz'.into());
//     let mut index = 0;
//     let len = components.len();

//     // Sorted alphabetically
//     let (fizz, write_fizz) = *components[0];
//     assert(fizz == 'Fizz'.into(), 'Fizz not found');
//     assert(write_fizz == false, 'Buzz should not write Fizz');

//     let (foo, write_foo) = *components[1];
//     assert(foo == 'Foo'.into(), 'Foo not found');
//     assert(write_foo == true, 'Buzz should write Foo');
// }

#[test]
#[available_gas(9000000)]
fn test_assume_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    // Assume FooWriter role
    let mut systems = ArrayTrait::new();
    systems.append('Bar'.into());
    world.assume_role('FooWriter'.into(), systems);

    // Get execution role
    let role = world.execution_role();
    assert(role == 'FooWriter'.into(), 'role not assumed');

    // Get systems for execution
    let is_system_for_execution = world.is_system_for_execution('Bar'.into());
    assert(is_system_for_execution == true, 'system not for execution');

    // Admin assumes Admin role
    let mut systems = ArrayTrait::new();
    systems.append('Bar'.into());
    world.assume_role(World::ADMIN.into(), systems);
}

#[test]
#[available_gas(9000000)]
#[should_panic]
fn test_assume_unauthorized_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Assume FooWriter role
    let mut systems = ArrayTrait::new();
    systems.append('Bar'.into());
    world.assume_role('FooWriter'.into(), systems);
}

#[test]
#[available_gas(9000000)]
fn test_clear_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    // Assume FooWriter role
    let mut systems = ArrayTrait::new();
    systems.append('Bar'.into());
    let cloned_systems = systems.clone();
    world.assume_role('FooWriter'.into(), systems);

    // Get execution role
    let role = world.execution_role();
    assert(role == 'FooWriter'.into(), 'role not assumed');

    // Get systems for execution
    let is_system_for_execution = world.is_system_for_execution('Bar'.into());
    assert(is_system_for_execution == true, 'system not for execution');

    // Clear role
    world.clear_role(cloned_systems);

    // Get execution role
    let role = world.execution_role();
    assert(role == 0.into(), 'role not cleared');

    // Get systems for execution
    let is_system_for_execution = world.is_system_for_execution('Bar'.into());
    assert(is_system_for_execution == false, 'system still for execution');
}

#[test]
#[available_gas(9000000)]
fn test_initialize() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    let is_authorized = world.is_authorized('Bar'.into(), 'Foo'.into(), AuthRole { id: role_id });
    assert(is_authorized, 'auth route not set');
}

#[test]
#[available_gas(5000000)]
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
#[available_gas(10000000)]
fn test_set_entity_authorized_with_assumed_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    // Assume FooWriter role
    let mut systems = ArrayTrait::new();
    systems.append('Bar'.into());
    world.assume_role('FooWriter'.into(), systems);

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
#[available_gas(10000000)]
fn test_set_entity_authorized_no_assumed_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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
    // No assumed role
    // Should pass since default scoped role is authorized (FooWriter)
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

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants Admin role to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append(World::ADMIN); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    // Call Bar system
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);

    // Assume Admin role
    let mut systems = ArrayTrait::<felt252>::new();
    world.assume_role(World::ADMIN.into(), systems);
    world.execute('Bar'.into(), data.span());

    // Assert that the data is stored
    // Caller here is the world contract via the executor
    let world_address = world.contract_address;
    let foo = world.entity('Foo'.into(), world_address.into(), 0, 0);
    assert(*foo[0] == 420, 'data not stored');
    assert(*foo[1] == 1337, 'data not stored');
}

#[test]
#[available_gas(11000000)]
#[should_panic]
fn test_admin_system_but_non_admin_caller() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin caller grants Admin role to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append(World::ADMIN); // role_id
    let systems = ArrayTrait::new();
    world.assume_role(World::ADMIN.into(), systems);
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    // Admin revokes its Admin role
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    let caller: felt252 = get_caller_address().into();
    grant_role_calldata.append(caller); // target_id
    world.execute('RevokeAuthRole'.into(), grant_role_calldata.span());
    let role = world.entity('AuthRole'.into(), QueryTrait::new_from_id(caller.into()), 0, 0);
    assert(*role[0] == 0, 'role not revoked');

    // Clear execution role
    let systems = ArrayTrait::new();
    world.clear_role(systems);

    // Non-admin calls an admin system
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('Bar'.into(), data.span());
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_unauthorized() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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
#[available_gas(10000000)]
#[should_panic]
fn test_set_entity_assumed_role_execute_another_system() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_system(Buzz::TEST_CLASS_HASH.try_into().unwrap());
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

    // Assume FooWriter role
    let mut systems = ArrayTrait::new();
    systems.append('Bar'.into());
    world.assume_role('FooWriter'.into(), systems);

    // Call Buzz system, different from the one when doing assumed role
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('Buzz'.into(), data.span());
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_directly() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    // Test context
    let ctx = Context {
        world,
        caller_account: contract_address_const::<0x1337>(),
        caller_system: 'Bar'.into(),
        execution_role: AuthRole {
            id: 'FooWriter'.into()
        },
    };

    // Change Foo component directly
    let id = world.uuid();
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.set_entity(ctx, 'Foo'.into(), QueryTrait::new_from_id(id.into()), 0, data.span());
}

#[test]
#[available_gas(9000000)]
fn test_grant_role() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Assume Admin role
    let mut systems = ArrayTrait::<felt252>::new();
    systems.append('Bar'.into());
    world.assume_role(World::ADMIN.into(), systems);

    // Admin caller grants FooWriter role to Bar system
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    grant_role_calldata.append('Bar'); // target_id
    grant_role_calldata.append('FooWriter'); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    // Assert that the role is set
    let role = world.entity('AuthRole'.into(), 'Bar'.into(), 0, 0);
    assert(*role[0] == 'FooWriter', 'role not granted');

    // Assume Admin role
    let mut systems = ArrayTrait::<felt252>::new();
    systems.append('Bar'.into());
    world.assume_role(World::ADMIN.into(), systems);

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

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Assume FooWriter role
    let mut systems = ArrayTrait::<felt252>::new();
    systems.append('Bar'.into());
    world.assume_role(World::ADMIN.into(), systems);

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

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

    world.register_system(Bar::TEST_CLASS_HASH.try_into().unwrap());
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

#[test]
#[available_gas(5000000)]
fn test_assume_admin_role_by_admin() {
    // Spawn empty world
    let world = spawn_empty_world();

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Assume Admin role by Admin
    let mut systems = ArrayTrait::<felt252>::new();
    world.assume_role(World::ADMIN.into(), systems);

    // Check that role is assumed
    assert(world.execution_role() == World::ADMIN.into(), 'role not assumed');
}

#[test]
#[available_gas(5000000)]
#[should_panic]
fn test_assume_admin_role_by_non_admin() {
    // Spawn empty world
    let world = spawn_empty_world();

    // No Auth route
    let mut route = ArrayTrait::new();

    // Initialize world
    world.initialize(route);

    // Admin revokes its Admin role
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();
    let caller: felt252 = get_caller_address().into();
    grant_role_calldata.append(caller); // target_id
    world.execute('RevokeAuthRole'.into(), grant_role_calldata.span());
    let role = world.entity('AuthRole'.into(), QueryTrait::new_from_id(caller.into()), 0, 0);
    assert(*role[0] == 0, 'role not revoked');

    // Non-admin assume Bar system that has Admin role
    // Should panic since caller is not admin anymore
    let mut systems = ArrayTrait::<felt252>::new();
    world.assume_role('Bar'.into(), systems);
}

// Utils
fn spawn_empty_world() -> IWorldDispatcher {
    // Deploy executor contract
    let executor_constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(),
        0,
        executor_constructor_calldata.span(),
        false
    )
        .unwrap();

    // Deploy world contract
    let mut constructor_calldata = array::ArrayTrait::new();
    constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        World::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();
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
    grant_role_calldata.append(World::ADMIN); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    world
}

#[test]
#[available_gas(6000000)]
fn test_library_call_system() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(LibraryCall::TEST_CLASS_HASH.try_into().unwrap());
    let mut calldata = ArrayTrait::new();
    calldata.append(FooComponent::TEST_CLASS_HASH);
    calldata.append(0x011efd13169e3bceace525b23b7f968b3cc611248271e35f04c5c917311fc7f7);
    calldata.append(0);
    world.execute('LibraryCall'.into(), calldata.span());
}
