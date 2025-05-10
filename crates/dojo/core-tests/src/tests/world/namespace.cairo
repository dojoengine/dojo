use dojo::utils::bytearray_hash;
use dojo::world::{IWorldDispatcherTrait, world};
use snforge_std::{EventSpyAssertionsTrait, spy_events};
use starknet::ContractAddress;
use dojo_snf_test;
use crate::tests::helpers::deploy_world;

#[test]
fn test_register_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    let mut spy = spy_events();

    let namespace = "namespace";
    let hash = bytearray_hash(@namespace);

    world.register_namespace(namespace.clone());

    assert(world.is_owner(hash, bob), 'namespace not registered');

    spy
        .assert_emitted(
            @array![
                (
                    world.contract_address,
                    world::Event::NamespaceRegistered(
                        world::NamespaceRegistered { namespace, hash },
                    ),
                ),
            ],
        );
}

#[test]
#[should_panic(expected: "Namespace `namespace` is already registered")]
fn test_register_namespace_already_registered_same_caller() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    world.register_namespace("namespace");
    world.register_namespace("namespace");
}

#[test]
#[should_panic(expected: "Namespace `namespace` is already registered")]
fn test_register_namespace_already_registered_other_caller() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    world.register_namespace("namespace");

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.register_namespace("namespace");
}

#[test]
#[should_panic(expected: "Namespace `` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$")]
fn test_register_namespace_empty_name() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_namespace("");
}
