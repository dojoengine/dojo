use starknet::{
    ClassHash, ContractAddress, syscalls::deploy_syscall, class_hash::Felt252TryIntoClassHash,
    get_caller_address
};
use array::{ArrayTrait, SpanTrait};
use traits::TryInto;
use option::OptionTrait;
use core::{result::ResultTrait, traits::Into};
use debug::PrintTrait;

use dojo::executor::executor;
use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait};
use dojo::packing::{shl, shr};

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
    let (contract, _) = starknet::deploy_syscall(class_hash.try_into().unwrap(), 0, calldata, false)
        .unwrap();
    contract
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
    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(),
        0,
        array![executor_address.into(), dojo::base::base::TEST_CLASS_HASH].span(),
        false
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


const GAS_OFFSET: felt252 = 0x1_000000_000000_000000_000000_000000; // 15 bajtów

/// Measures gas used after previous measurement and prints it
///
/// # Arguments
///
/// * `start` - gas before measurement
/// * `name` - name of test, at most 15 bytes, will be padded with spaces
fn end(start: u128, name: felt252) {
    let gas_after = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let mut name: u256 = name.into();

    // overwriting zeros with spaces
    let mut char = 0;
    loop {
        if char == 15 {
            break;
        }
        // if given byte is zero
        if shl(0xff, 8 * char) & name == 0 {
            name = name | shl(0x20, 8 * char); // set space
        }
        char += 1;
    };

    let name: felt252 = (name % GAS_OFFSET.into()).try_into().unwrap();
    let used_gas = (start - gas_after - 1770).into() * GAS_OFFSET;
    (used_gas + name).print();
}
