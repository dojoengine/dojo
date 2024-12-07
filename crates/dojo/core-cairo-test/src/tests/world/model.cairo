use core::starknet::ContractAddress;

use crate::tests::helpers::{
    Foo, m_Foo, DOJO_NSH, drop_all_events, deploy_world, deploy_world_for_model_upgrades,
    foo_invalid_name
};
use dojo::world::{world, IWorldDispatcherTrait};
use dojo::model::Model;


#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelBadLayoutType {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub c: u256,
    pub d: u256
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub a: felt252,
    pub c: u256
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
    pub c: u256
}

#[test]
fn test_register_model_for_namespace_owner() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ModelRegistered(event) = event.unwrap() {
        assert(event.name == Model::<Foo>::name(), 'bad event name');
        assert(event.namespace == "dojo", 'bad event namespace');
        assert(
            event.class_hash == m_Foo::TEST_CLASS_HASH.try_into().unwrap(), 'bad event class_hash'
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad event prev address'
        );
    } else {
        core::panic_with_felt252('no ModelRegistered event');
    }

    assert(world.is_owner(Model::<Foo>::selector(DOJO_NSH), bob), 'bob is not the owner');
}


#[test]
#[should_panic(
    expected: (
        "Name `foo-bis` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_register_model_with_invalid_name() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_model("dojo", foo_invalid_name::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: ("Account `2827` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED',)
)]
fn test_register_model_for_namespace_writer() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_writer(DOJO_NSH, bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_upgrade_model_from_model_owner() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world_for_model_upgrades();
    world.grant_owner(Model::<FooModelMemberAdded>::selector(DOJO_NSH), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    world.upgrade_model("dojo", m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ModelUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Model::<FooModelMemberAdded>::selector(DOJO_NSH), 'bad model selector'
        );
        assert(
            event.class_hash == m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap(),
            'bad model class_hash'
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad model prev address'
        );
    } else {
        core::panic_with_felt252('no ModelUpgraded event');
    }

    assert(
        world.is_owner(Model::<FooModelMemberAdded>::selector(DOJO_NSH), bob),
        'bob is not the owner'
    );
}

#[test]
fn test_upgrade_model() {
    let world = deploy_world_for_model_upgrades();

    drop_all_events(world.contract_address);

    world.upgrade_model("dojo", m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ModelUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Model::<FooModelMemberAdded>::selector(DOJO_NSH), 'bad model selector'
        );
        assert(
            event.class_hash == m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap(),
            'bad model class_hash'
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(), 'bad model address'
        );
    } else {
        core::panic_with_felt252('no ModelUpgraded event');
    }
}

#[test]
#[should_panic(
    expected: (
        "Invalid new layout to upgrade the resource `dojo-FooModelBadLayoutType`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_model_with_bad_layout_type() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", m_FooModelBadLayoutType::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooModelMemberRemoved`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_model_with_member_removed() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", m_FooModelMemberRemoved::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooModelMemberAddedButRemoved`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_model_with_member_added_but_removed() {
    let world = deploy_world_for_model_upgrades();
    world
        .upgrade_model(
            "dojo", m_FooModelMemberAddedButRemoved::TEST_CLASS_HASH.try_into().unwrap()
        );
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooModelMemberAddedButMoved`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_model_with_member_moved() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", m_FooModelMemberAddedButMoved::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on model (or its namespace) `FooModelMemberAdded`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_model_from_model_writer() {
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world_for_model_upgrades();

    world.grant_writer(Model::<FooModelMemberAdded>::selector(DOJO_NSH), alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.upgrade_model("dojo", m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Resource `dojo-Foo` is already registered", 'ENTRYPOINT_FAILED',))]
fn test_upgrade_model_from_random_account() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);
    world.grant_owner(DOJO_NSH, alice);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Namespace `another_namespace` is not registered", 'ENTRYPOINT_FAILED',))]
fn test_register_model_with_unregistered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_model("another_namespace", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

// It's CONTRACT_NOT_DEPLOYED for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the register_model function.
#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED',))]
fn test_register_model_through_malicious_contract() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}
