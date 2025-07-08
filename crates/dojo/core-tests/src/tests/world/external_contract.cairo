use dojo::utils::selector_from_namespace_and_name;
use dojo::world::{IWorldDispatcherTrait, world};
use dojo_snf_test::declare_deploy::{declare, deploy};
use snforge_std::{EventSpyAssertionsTrait, spy_events};
use starknet::ContractAddress;
use crate::tests::helpers::{DOJO_NSH, deploy_world, deploy_world_and_foo};

#[starknet::contract]
mod ERC20 {
    #[storage]
    struct Storage {}
}

#[test]
fn test_register_external_contract() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    dojo_snf_test::set_block_number(123);

    let (class, class_hash) = declare("ERC20");
    let token_address = deploy(class, @array![]);

    let namespace = "dojo";
    let contract_name = "ERC20";
    let instance_name = "GoldToken";
    let selector = selector_from_namespace_and_name(DOJO_NSH, @instance_name);
    let block_number = 123_u64;

    let mut spy = spy_events();

    world
        .register_external_contract(
            namespace.clone(),
            contract_name.clone(),
            instance_name.clone(),
            token_address,
            block_number,
        );

    assert(world.is_owner(selector, bob), 'ext. contract not registered');

    spy
        .assert_emitted(
            @array![
                (
                    world.contract_address,
                    world::Event::ExternalContractRegistered(
                        world::ExternalContractRegistered {
                            namespace,
                            contract_name,
                            instance_name,
                            contract_selector: selector,
                            class_hash,
                            contract_address: token_address,
                            block_number,
                        },
                    ),
                ),
            ],
        );
}

#[test]
#[should_panic(
    expected: "Resource (External Contract) `dojo-GoldToken (ERC20)` is already registered",
)]
fn test_register_already_registered_external_contract() {
    let world = deploy_world();
    let world = world.dispatcher;

    let token_address: ContractAddress = 'gold'.try_into().unwrap();

    world.register_external_contract("dojo", "ERC20", "GoldToken", token_address, 0);
    world.register_external_contract("dojo", "ERC20", "GoldToken", token_address, 0);
}

#[test]
#[should_panic(expected: "Namespace `dojo2` is not registered")]
fn test_register_external_contract_in_a_not_registered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_external_contract("dojo2", "ERC20", "GoldToken", 'gold'.try_into().unwrap(), 0);
}

#[test]
#[should_panic(expected: "Account `0xb0b` does NOT have OWNER role on namespace `dojo`")]
fn test_register_external_contract_without_owner_permission_on_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    world.register_external_contract("dojo", "ERC20", "GoldToken", 'gold'.try_into().unwrap(), 0);
}

#[test]
fn test_upgrade_external_contract() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    dojo_snf_test::set_block_number(123);

    let (class, class_hash) = declare("ERC20");
    let new_token_address = deploy(class, @array![]);

    let namespace = "dojo";
    let contract_name = "ERC20";
    let instance_name = "GoldToken";
    let selector = selector_from_namespace_and_name(DOJO_NSH, @instance_name);
    let block_number = 123_u64;

    world
        .register_external_contract(
            namespace.clone(),
            contract_name.clone(),
            instance_name.clone(),
            'gold'.try_into().unwrap(),
            0,
        );

    let mut spy = spy_events();

    world
        .upgrade_external_contract(
            namespace.clone(), instance_name.clone(), new_token_address, block_number,
        );

    spy
        .assert_emitted(
            @array![
                (
                    world.contract_address,
                    world::Event::ExternalContractUpgraded(
                        world::ExternalContractUpgraded {
                            namespace,
                            instance_name,
                            contract_selector: selector,
                            contract_address: new_token_address,
                            class_hash,
                            block_number,
                        },
                    ),
                ),
            ],
        );
}

#[test]
#[should_panic(
    expected: "Account `0xb0b` does NOT have OWNER role on external contract (at 0x676f6c64)",
)]
fn test_upgrade_external_contract_without_owner_permission() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_external_contract("dojo", "ERC20", "GoldToken", 'gold'.try_into().unwrap(), 0);

    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    world.upgrade_external_contract("dojo", "GoldToken", 'new_gold'.try_into().unwrap(), 0);
}

#[test]
#[should_panic(expected: "Resource `dojo-GoldToken` is not registered")]
fn test_upgrade_external_contract_without_being_registered_first() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.upgrade_external_contract("dojo", "GoldToken", 'new_gold'.try_into().unwrap(), 0);
}

#[test]
#[should_panic(expected: "Resource `dojo-Foo` is registered but not as external contract")]
fn test_upgrade_external_contract_with_already_registered_resource_conflict() {
    let (world, _) = deploy_world_and_foo();
    let world = world.dispatcher;

    world.upgrade_external_contract("dojo", "Foo", 'new_gold'.try_into().unwrap(), 0);
}
