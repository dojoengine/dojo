use dojo::world::{world, IWorldDispatcherTrait};
use dojo::utils::selector_from_namespace_and_name;

use crate::tests::helpers::{DOJO_NSH, drop_all_events, deploy_world, deploy_world_and_foo};

#[test]
fn test_register_external_contract() {
    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let world = deploy_world();
    let world = world.dispatcher;

    let token_address = starknet::contract_address_const::<'gold'>();
    let namespace = "dojo";
    let contract_name = "ERC20";
    let instance_name = "GoldToken";
    let selector = selector_from_namespace_and_name(DOJO_NSH, @instance_name);

    drop_all_events(world.contract_address);

    world
        .register_external_contract(
            namespace.clone(), contract_name.clone(), instance_name.clone(), token_address,
        );

    assert(world.is_owner(selector, bob), 'ext. contract not registered');

    match starknet::testing::pop_log::<world::Event>(world.contract_address).unwrap() {
        world::Event::ExternalContractRegistered(event) => {
            assert(event.namespace == namespace, 'bad namespace');
            assert(event.contract_name == contract_name, 'bad contract name');
            assert(event.instance_name == instance_name, 'bad instance name');
            assert(event.contract_selector == selector, 'bad contract selector');
            assert(event.contract_address == token_address, 'bad contract address');
            // TODO: add class_hash check after migration to snfoundry
        },
        _ => panic!("no ExternalContractRegistered event"),
    }
}

#[test]
#[should_panic(
    expected: (
        "Resource (External Contract) `dojo-GoldToken (ERC20)` is already registered",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_register_already_registered_external_contract() {
    let world = deploy_world();
    let world = world.dispatcher;

    world
        .register_external_contract(
            "dojo", "ERC20", "GoldToken", starknet::contract_address_const::<'gold'>(),
        );

    world
        .register_external_contract(
            "dojo", "ERC20", "GoldToken", starknet::contract_address_const::<'gold'>(),
        );
}

#[test]
#[should_panic(expected: ("Namespace `dojo2` is not registered", 'ENTRYPOINT_FAILED'))]
fn test_register_external_contract_in_a_not_registered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world
        .register_external_contract(
            "dojo2", "ERC20", "GoldToken", starknet::contract_address_const::<'gold'>(),
        );
}

#[test]
#[should_panic(
    expected: ("Account `2827` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED'),
)]
fn test_register_external_contract_without_owner_permission_on_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world
        .register_external_contract(
            "dojo", "ERC20", "GoldToken", starknet::contract_address_const::<'gold'>(),
        );
}

#[test]
fn test_upgrade_external_contract() {
    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let world = deploy_world();
    let world = world.dispatcher;

    let token_address = starknet::contract_address_const::<'gold'>();
    let new_token_address = starknet::contract_address_const::<'new_gold'>();
    let namespace = "dojo";
    let contract_name = "ERC20";
    let instance_name = "GoldToken";
    let selector = selector_from_namespace_and_name(DOJO_NSH, @instance_name);

    drop_all_events(world.contract_address);

    world
        .register_external_contract(
            namespace.clone(), contract_name.clone(), instance_name.clone(), token_address,
        );

    drop_all_events(world.contract_address);

    world.upgrade_external_contract(namespace.clone(), instance_name.clone(), new_token_address);

    match starknet::testing::pop_log::<world::Event>(world.contract_address).unwrap() {
        world::Event::ExternalContractUpgraded(event) => {
            assert(event.namespace == namespace, 'bad namespace');
            assert(event.instance_name == instance_name, 'bad instance name');
            assert(event.contract_selector == selector, 'bad contract selector');
            assert(event.contract_address == new_token_address, 'bad contract address');
            // TODO: add class_hash check after migration to snfoundry
        },
        _ => panic!("no ExternalContractUpgraded event"),
    }
}

#[test]
#[should_panic(
    expected: (
        "Account `2827` does NOT have OWNER role on external contract (at 0x676f6c64)",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_external_contract_without_owner_permission() {
    let world = deploy_world();
    let world = world.dispatcher;

    let token_address = starknet::contract_address_const::<'gold'>();
    let new_token_address = starknet::contract_address_const::<'new_gold'>();
    let namespace = "dojo";
    let contract_name = "ERC20";
    let instance_name = "GoldToken";

    drop_all_events(world.contract_address);

    world
        .register_external_contract(
            namespace.clone(), contract_name.clone(), instance_name.clone(), token_address,
        );

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.upgrade_external_contract(namespace.clone(), instance_name.clone(), new_token_address);
}

#[test]
#[should_panic(expected: ("Resource `dojo-GoldToken` is not registered", 'ENTRYPOINT_FAILED'))]
fn test_upgrade_external_contract_without_being_registered_first() {
    let world = deploy_world();
    let world = world.dispatcher;

    world
        .upgrade_external_contract(
            "dojo", "GoldToken", starknet::contract_address_const::<'new_gold'>(),
        );
}

#[test]
#[should_panic(
    expected: (
        "Resource `dojo-Foo` is registered but not as external contract", 'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_external_contract_with_already_registered_resource_conflict() {
    let (world, _) = deploy_world_and_foo();
    let world = world.dispatcher;

    world
        .upgrade_external_contract("dojo", "Foo", starknet::contract_address_const::<'new_gold'>());
}
