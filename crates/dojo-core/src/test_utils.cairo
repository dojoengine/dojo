use starknet::{ClassHash, syscalls::deploy_syscall, class_hash::Felt252TryIntoClassHash, get_caller_address};
use array::ArrayTrait;
use traits::TryInto;
use option::OptionTrait;
use core::{result::ResultTrait, traits::Into};

use dojo_core::{executor::Executor, world::World, interfaces::{IWorldDispatcher, IWorldDispatcherTrait}};
use dojo_core::auth::components::{AuthRoleComponent, AuthStatusComponent};
use dojo_core::auth::systems::{
    Route, RouteAuthSystem, IsAuthorizedSystem, IsAccountAdminSystem, GrantAuthRoleSystem,
    RevokeAuthRoleSystem, GrantResourceSystem, RevokeResourceSystem, GrantScopedAuthRoleSystem,
    RevokeScopedAuthRoleSystem
};

fn spawn_test_world(
    components: Array<felt252>, systems: Array<felt252>, routes: Array<Route>
) -> IWorldDispatcher {
    // deploy executor
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();

    // deploy world
    let mut world_constructor_calldata = array::ArrayTrait::new();
    world_constructor_calldata.append('World');
    world_constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        World::TEST_CLASS_HASH.try_into().unwrap(), 0, world_constructor_calldata.span(), false
    ).unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

    // register auth components and systems
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

    // Grant Admin role to the spawner
    let caller = get_caller_address();
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();

    grant_role_calldata.append(caller.into()); // target_id
    grant_role_calldata.append('Admin'); // role_id
    world.execute('GrantAuthRole'.into(), grant_role_calldata.span());

    // register components
    let mut index = 0;
    loop {
        if index == components.len() {
            break ();
        }
        world.register_component((*components[index]).try_into().unwrap());
        index += 1;
    };

    // register systems
    let mut index = 0;
    loop {
        if index == systems.len() {
            break ();
        }
        world.register_system((*systems[index]).try_into().unwrap());
        index += 1;
    };

    // initialize world by setting the auth routes
    world.initialize(routes);

    world
}

// Creates auth components and systems for testing
fn mock_auth_components_systems() -> (Array<ClassHash>, Array<ClassHash>) {
    // Auth components
    let mut components = array::ArrayTrait::new();
    components.append(AuthRoleComponent::TEST_CLASS_HASH.try_into().unwrap());
    components.append(AuthStatusComponent::TEST_CLASS_HASH.try_into().unwrap());

    // Auth systems
    let mut systems = array::ArrayTrait::new();
    systems.append(RouteAuthSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(IsAuthorizedSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(IsAccountAdminSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(GrantAuthRoleSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(RevokeAuthRoleSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(GrantResourceSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(RevokeResourceSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(GrantScopedAuthRoleSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(RevokeScopedAuthRoleSystem::TEST_CLASS_HASH.try_into().unwrap());

    (components, systems)
}
