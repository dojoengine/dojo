use dojo::world::{world, IWorldDispatcherTrait};
use dojo::utils::bytearray_hash;

use crate::tests::helpers::{drop_all_events, deploy_world};

#[test]
fn test_register_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    let namespace = "namespace";
    let hash = bytearray_hash(@namespace);

    world.register_namespace(namespace.clone());

    assert(world.is_owner(hash, bob), 'namespace not registered');

    match starknet::testing::pop_log::<world::Event>(world.contract_address).unwrap() {
        world::Event::NamespaceRegistered(event) => {
            assert(event.namespace == namespace, 'bad namespace');
            assert(event.hash == hash, 'bad hash');
        },
        _ => panic!("no NamespaceRegistered event"),
    }
}

#[test]
#[should_panic(expected: ("Namespace `namespace` is already registered", 'ENTRYPOINT_FAILED',))]
fn test_register_namespace_already_registered_same_caller() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_namespace("namespace");
    world.register_namespace("namespace");
}

#[test]
#[should_panic(expected: ("Namespace `namespace` is already registered", 'ENTRYPOINT_FAILED',))]
fn test_register_namespace_already_registered_other_caller() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_namespace("namespace");

    let alice = starknet::contract_address_const::<0xa11ce>();
    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.register_namespace("namespace");
}


#[test]
#[available_gas(6000000)]
#[should_panic(
    expected: (
        "Namespace `` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_register_namespace_empty_name() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_namespace("");
}
