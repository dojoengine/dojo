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

/// Deploy classhash with calldata for constructor
///
/// # Arguments
///
/// * `class_hash` - Class to deploy
/// * `calldata` - calldata for constructor
///
/// # Returns
/// * address of contract deployed
fn deploy_contract(class_hash: felt252, calldata: Span<felt252>) -> ContractAddress {
    let (system_contract, _) = starknet::deploy_syscall(
        class_hash.try_into().unwrap(), 0, calldata, false
    )
        .unwrap();
    system_contract
}

/// Deploy classhash and passes in world address to constructor
///
/// # Arguments
///
/// * `class_hash` - Class to deploy
/// * `world` - World dispatcher to pass as world address
///
/// # Returns
/// * address of contract deployed
fn deploy_with_world_address(class_hash: felt252, world: IWorldDispatcher) -> ContractAddress {
    deploy_contract(class_hash, array![world.contract_address.into()].span())
}

fn spawn_test_world(models: Array<felt252>) -> IWorldDispatcher {
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

    // register models
    let mut index = 0;
    loop {
        if index == models.len() {
            break ();
        }
        world.register_model((*models[index]).try_into().unwrap());
        index += 1;
    };

    world
}
