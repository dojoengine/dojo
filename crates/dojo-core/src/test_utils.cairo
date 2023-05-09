use starknet::ClassHash;
use starknet::syscalls::deploy_syscall;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::get_caller_address;

use array::ArrayTrait;
use traits::TryInto;
use option::OptionTrait;
use core::result::ResultTrait;
use core::traits::Into;

use dojo_core::executor::Executor;
use dojo_core::world::World;
use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::interfaces::IWorldDispatcherTrait;

use dojo_core::auth::components::{RoleComponent, StatusComponent};
use dojo_core::auth::systems::{
    RouteAuthSystem, AuthorizeSystem, GrantRoleSystem, RevokeRoleSystem, 
    GrantResourceSystem, RevokeResourceSystem
};

fn spawn_test_world(components: Array<felt252>, systems: Array<felt252>) -> IWorldDispatcher {
    // deploy executor
    let constructor_calldata = array::ArrayTrait::<felt252>::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();

    // deploy world
    let mut world_constructor_calldata = array::ArrayTrait::<felt252>::new();
    world_constructor_calldata.append('World');
    world_constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        World::TEST_CLASS_HASH.try_into().unwrap(), 0, world_constructor_calldata.span(), false
    ).unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

    // register auth components and systems
    let (auth_components, auth_systems) = mock_auth_components_systems();
    let mut auth_components_index = 0;
    loop {
        if auth_components_index == auth_components.len() {
            break ();
        }
        world.register_component(*auth_components.at(auth_components_index));
        auth_components_index += 1;
    };

    let mut auth_systems_index = 0;
    loop {
        if auth_systems_index == auth_systems.len() {
            break ();
        }
        world.register_system(*auth_systems.at(auth_systems_index));
        auth_systems_index += 1;
    };

    // register components
    let mut components_index = 0;
    loop {
        if components_index == components.len() {
            break ();
        }
        world.register_component((*components[components_index]).try_into().unwrap());
        components_index+=1;
    };

    // register systems
    let mut systems_index = 0;
    loop {
        if systems_index == systems.len() {
            break ();
        }
        world.register_system((*systems[systems_index]).try_into().unwrap());
        systems_index+=1;
    };

    // Grant Admin role to the spawner
    let caller = get_caller_address();
    let mut grant_role_calldata: Array<felt252> = ArrayTrait::new();

    grant_role_calldata.append(caller.into()); // target_id
    grant_role_calldata.append('Admin'); // role_id
    world.execute('GrantRole'.into(), grant_role_calldata.span());

    world
}

// Creates auth components and systems for testing
fn mock_auth_components_systems() -> (Array<ClassHash>, Array<ClassHash>) {
    // Auth components
    let mut components = array::ArrayTrait::<ClassHash>::new();
    components.append(RoleComponent::TEST_CLASS_HASH.try_into().unwrap());
    components.append(StatusComponent::TEST_CLASS_HASH.try_into().unwrap());

    // Auth systems
    let mut systems = array::ArrayTrait::<ClassHash>::new();
    systems.append(RouteAuthSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(AuthorizeSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(GrantRoleSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(RevokeRoleSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(GrantResourceSystem::TEST_CLASS_HASH.try_into().unwrap());
    systems.append(RevokeResourceSystem::TEST_CLASS_HASH.try_into().unwrap());

    (components, systems)
}
