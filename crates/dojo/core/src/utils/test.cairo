use core::array::SpanTrait;
use core::option::OptionTrait;
use core::result::ResultTrait;
use core::traits::{Into, TryInto};

use starknet::{ContractAddress, syscalls::deploy_syscall};

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
pub fn deploy_contract(class_hash: felt252, calldata: Span<felt252>) -> ContractAddress {
    let (contract, _) = starknet::syscalls::deploy_syscall(
        class_hash.try_into().unwrap(), 0, calldata, false
    )
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
pub fn deploy_with_world_address(class_hash: felt252, world: IWorldDispatcher) -> ContractAddress {
    deploy_contract(class_hash, [world.contract_address.into()].span())
}

/// Spawns a test world registering namespaces and models.
///
/// # Arguments
///
/// * `namespaces` - Namespaces to register.
/// * `models` - Models to register.
///
/// # Returns
///
/// * World dispatcher
pub fn spawn_test_world(namespaces: Span<ByteArray>, models: Span<felt252>) -> IWorldDispatcher {
    let salt = core::testing::get_available_gas();

    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(),
        salt.into(),
        [world::TEST_CLASS_HASH].span(),
        false
    )
        .unwrap();

    let world = IWorldDispatcher { contract_address: world_address };

    // Register all namespaces to ensure correct registration of models.
    let mut namespaces = namespaces;
    while let Option::Some(namespace) = namespaces.pop_front() {
        world.register_namespace(namespace.clone());
    };

    let dummy_ns = "dummy";

    // Register all models.
    let mut index = 0;
    loop {
        if index == models.len() {
            break ();
        }
        world.register_model(dummy_ns.clone(), (*models[index]).try_into().unwrap());
        index += 1;
    };

    world
}

#[derive(Drop)]
pub struct GasCounter {
    pub start: u128,
}

#[generate_trait]
pub impl GasCounterImpl of GasCounterTrait {
    fn start() -> GasCounter {
        let start = core::testing::get_available_gas();
        core::gas::withdraw_gas().unwrap();
        GasCounter { start }
    }

    fn end(self: GasCounter, name: ByteArray) {
        let end = core::testing::get_available_gas();
        let gas_used = self.start - end;

        println!("# GAS # {}: {}", Self::pad_start(name, 18), gas_used);
        core::gas::withdraw_gas().unwrap();
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
pub fn assert_array(value: Span<felt252>, expected: Span<felt252>) {
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
