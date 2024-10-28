use core::starknet::ContractAddress;
use dojo::world::{world, IWorldDispatcherTrait};
use dojo::meta::{IDeployedResourceDispatcher, IDeployedResourceDispatcherTrait};
use crate::tests::helpers::{DOJO_NSH, test_contract, drop_all_events, deploy_world};

#[test]
fn test_deploy_contract_for_namespace_owner() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_owner(DOJO_NSH, bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    let contract_address = world.register_contract('salt1', "dojo", class_hash);

    let event = match starknet::testing::pop_log::<world::Event>(world.contract_address).unwrap() {
        world::Event::ContractRegistered(event) => event,
        _ => panic!("no ContractRegistered event"),
    };

    let contract = IDeployedResourceDispatcher { contract_address };
    let contract_name = contract.dojo_name();

    assert(event.name == contract_name, 'bad name');
    assert(event.namespace == "dojo", 'bad namespace');
    assert(event.salt == 'salt1', 'bad event salt');
    assert(event.class_hash == class_hash, 'bad class_hash');
    assert(
        event.address != core::num::traits::Zero::<ContractAddress>::zero(), 'bad contract address'
    );
}

#[test]
#[should_panic(
    expected: ("Account `2827` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED',)
)]
fn test_deploy_contract_for_namespace_writer() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_writer(DOJO_NSH, bob);

    // the account has write access to the 'test_contract' namespace so it should be able to deploy
    // the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
}


#[test]
#[should_panic(
    expected: ("Account `2827` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED',)
)]
fn test_deploy_contract_no_namespace_owner_access() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Namespace `buzz_namespace` is not registered", 'ENTRYPOINT_FAILED',))]
fn test_deploy_contract_with_unregistered_namespace() {
    let world = deploy_world();
    world
        .register_contract(
            'salt1', "buzz_namespace", test_contract::TEST_CLASS_HASH.try_into().unwrap()
        );
}

// It's CONTRACT_NOT_DEPLOYED for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the deploy_contract function.
#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED',))]
fn test_deploy_contract_through_malicious_contract() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(DOJO_NSH, bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);

    world.register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_upgrade_contract_from_resource_owner() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let contract_address = world.register_contract('salt1', "dojo", class_hash);
    let contract = IDeployedResourceDispatcher { contract_address };
    let contract_name = contract.dojo_name();

    drop_all_events(world.contract_address);

    world.upgrade_contract("dojo", class_hash);

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ContractUpgraded(event) = event.unwrap() {
        assert(
            event
                .selector == dojo::utils::selector_from_namespace_and_name(
                    DOJO_NSH, @contract_name
                ),
            'bad contract selector'
        );
        assert(event.class_hash == class_hash, 'bad class_hash');
    } else {
        core::panic_with_felt252('no ContractUpgraded event');
    };
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on contract (or its namespace) `test_contract`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_contract_from_resource_writer() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let contract_address = world.register_contract('salt1', "dojo", class_hash);
    let contract = IDeployedResourceDispatcher { contract_address };
    let contract_name = contract.dojo_name();
    let contract_selector = dojo::utils::selector_from_namespace_and_name(DOJO_NSH, @contract_name);

    world.grant_writer(contract_selector, alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.upgrade_contract("dojo", class_hash);
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on contract (or its namespace) `test_contract`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_contract_from_random_account() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let _contract_address = world.register_contract('salt1', "dojo", class_hash);

    let alice = starknet::contract_address_const::<0xa11ce>();

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.upgrade_contract("dojo", class_hash);
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED',))]
fn test_upgrade_contract_through_malicious_contract() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let _contract_address = world.register_contract('salt1', "dojo", class_hash);

    starknet::testing::set_contract_address(malicious_contract);

    world.upgrade_contract("dojo", class_hash);
}
