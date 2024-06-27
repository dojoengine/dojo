use starknet::{
    ClassHash, ContractAddress, syscalls::deploy_syscall, class_hash::Felt252TryIntoClassHash,
    get_caller_address
};
use array::{ArrayTrait, SpanTrait};
use traits::TryInto;
use option::OptionTrait;
use core::{result::ResultTrait, traits::Into};
use debug::PrintTrait;

use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait};
use dojo::packing::{shl, shr};
use dojo::resource_metadata::resource_metadata;

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
    let salt = testing::get_available_gas();

    // deploy world
    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(),
        salt.into(),
        array![dojo::base::base::TEST_CLASS_HASH].span(),
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


#[derive(Drop)]
struct GasCounter {
    start: u128,
}

#[generate_trait]
impl GasCounterImpl of GasCounterTrait {
    fn start() -> GasCounter {
        let start = testing::get_available_gas();
        gas::withdraw_gas().unwrap();
        GasCounter { start }
    }

    fn end(self: GasCounter, name: ByteArray) {
        let end = testing::get_available_gas();
        let gas_used = self.start - end;

        println!("# GAS # {}: {}", Self::pad_start(name, 18), gas_used);
        gas::withdraw_gas().unwrap();
    }

    fn pad_start(str: ByteArray, len: u32) -> ByteArray {
        let mut missing: ByteArray = "";
        let missing_len = if str.len() >= len {
            0
        } else {
            len - str.len()
        };

        while missing.len() < missing_len {
            missing.append(@".");
        };
        missing + str
    }
}

// assert that `value` and `expected` have the same size and the same content
fn assert_array(value: Span<felt252>, expected: Span<felt252>) {
    assert!(value.len() == expected.len(), "Bad array length");

    let mut i = 0;
    loop {
        if i >= value.len() {
            break;
        }

        assert!(
            *value.at(i) == *expected.at(i),
            "Bad array value [{}] (expected: {} got: {})",
            i,
            *expected.at(i),
            *value.at(i)
        );

        i += 1;
    }
}
