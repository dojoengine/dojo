use starknet::{
    ClassHash, ContractAddress, syscalls::deploy_syscall, class_hash::Felt252TryIntoClassHash,
    get_caller_address
};
use array::{ArrayTrait, SpanTrait};
use traits::TryInto;
use option::OptionTrait;
use core::{result::ResultTrait, traits::Into};

use dojo::executor::executor;
use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait};

fn spawn_test_world(components: Array<felt252>, systems: Array<felt252>) -> IWorldDispatcher {
    // deploy executor
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();

    // deploy world
    let mut world_constructor_calldata = array::ArrayTrait::new();
    world_constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(), 0, world_constructor_calldata.span(), false
    )
        .unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

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

    world
}
