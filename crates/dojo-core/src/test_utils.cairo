use starknet::ClassHash;
use starknet::syscalls::deploy_syscall;
use starknet::class_hash::Felt252TryIntoClassHash;

use array::ArrayTrait;
use traits::TryInto;
use option::OptionTrait;
use core::result::ResultTrait;
use core::traits::Into;

use dojo_core::executor::Executor;
use dojo_core::world::World;
use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::interfaces::IWorldDispatcherTrait;

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

    world
}