use core::option::OptionTrait;
use core::result::ResultTrait;
use core::traits::{Into, TryInto};

use starknet::{ContractAddress, ClassHash, syscalls::deploy_syscall};

use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait};

/// In Cairo test runner, all the classes are expected to be declared already.
/// If a contract belong to an other crate, it must be added to the `build-external-contract`,
/// event for testing, since Scarb does not do that automatically anymore.
#[derive(Drop)]
pub enum TestResource {
    Event: ClassHash,
    Model: ClassHash,
    Contract: ClassHash,
}

#[derive(Drop)]
pub struct NamespaceDef {
    pub namespace: ByteArray,
    pub resources: Span<TestResource>,
}

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

/// Spawns a test world registering provided resources into namespaces.
///
/// # Arguments
///
/// * `namespaces_defs` - Definitions of namespaces to register.
///
/// # Returns
///
/// * World dispatcher
pub fn spawn_test_world(namespaces_defs: Span<NamespaceDef>) -> IWorldDispatcher {
    let salt = core::testing::get_available_gas();

    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(),
        salt.into(),
        [world::TEST_CLASS_HASH].span(),
        false
    )
        .unwrap();

    let world = IWorldDispatcher { contract_address: world_address };

    for ns in namespaces_defs {
        let namespace = ns.namespace.clone();
        world.register_namespace(namespace.clone());

        for r in ns
            .resources
            .clone() {
                match r {
                    TestResource::Event(ch) => {
                        world.register_event(namespace.clone(), *ch);
                    },
                    TestResource::Model(ch) => {
                        world.register_model(namespace.clone(), *ch);
                    },
                    TestResource::Contract(ch) => {
                        let salt: felt252 = (*ch).into();
                        world.register_contract(salt, namespace.clone(), *ch);
                    },
                }
            }
    };

    world
}
