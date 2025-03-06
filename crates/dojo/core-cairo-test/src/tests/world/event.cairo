use core::starknet::ContractAddress;

use crate::tests::helpers::{
    SimpleEvent, e_SimpleEvent, DOJO_NSH, e_FooEventBadLayoutType, drop_all_events, deploy_world,
    deploy_world_for_event_upgrades,
};
use dojo::world::{world, IWorldDispatcherTrait};
use dojo::event::Event;

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
pub struct FooEventMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
pub struct FooEventMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub c: u256,
    pub d: u256,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
pub struct FooEventMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub a: felt252,
    pub c: u256,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
pub struct FooEventMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
    pub c: u256,
}

#[derive(Introspect, Copy, Drop, Serde, PartialEq, Default)]
enum MyEnum {
    #[default]
    X: u8,
    Y: u16,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::event]
struct FooEventMemberChanged {
    #[key]
    pub caller: ContractAddress,
    pub a: (MyEnum, u8, u32),
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde, Default)]
enum AnotherEnum {
    #[default]
    X: bool,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::event]
struct FooEventMemberIllegalChange {
    #[key]
    pub caller: ContractAddress,
    pub a: MyEnum,
    pub b: u128,
}

#[test]
fn test_register_event_for_namespace_owner() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_event("dojo", e_SimpleEvent::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::EventRegistered(event) = event.unwrap() {
        assert(event.name == Event::<SimpleEvent>::name(), 'bad event name');
        assert(event.namespace == "dojo", 'bad event namespace');
        assert(
            event.class_hash == e_SimpleEvent::TEST_CLASS_HASH.try_into().unwrap(),
            'bad event class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad event prev address',
        );
    } else {
        core::panic_with_felt252('no EventRegistered event');
    }

    assert(world.is_owner(Event::<SimpleEvent>::selector(DOJO_NSH), bob), 'bob is not the owner');
}

#[test]
#[should_panic(
    expected: ("Account `2827` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED'),
)]
fn test_register_event_for_namespace_writer() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_writer(DOJO_NSH, bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_event("dojo", e_SimpleEvent::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_upgrade_event_from_event_owner() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world_for_event_upgrades();
    world.grant_owner(Event::<FooEventMemberAdded>::selector(DOJO_NSH), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    world.upgrade_event("dojo", e_FooEventMemberAdded::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::EventUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Event::<FooEventMemberAdded>::selector(DOJO_NSH),
            'bad model selector',
        );
        assert(
            event.class_hash == e_FooEventMemberAdded::TEST_CLASS_HASH.try_into().unwrap(),
            'bad model class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad model prev address',
        );
    } else {
        core::panic_with_felt252('no EventUpgraded event');
    }

    assert(
        world.is_owner(Event::<FooEventMemberAdded>::selector(DOJO_NSH), bob),
        'bob is not the owner',
    );
}

#[test]
fn test_upgrade_event() {
    let world = deploy_world_for_event_upgrades();

    drop_all_events(world.contract_address);

    world.upgrade_event("dojo", e_FooEventMemberAdded::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::EventUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Event::<FooEventMemberAdded>::selector(DOJO_NSH),
            'bad model selector',
        );
        assert(
            event.class_hash == e_FooEventMemberAdded::TEST_CLASS_HASH.try_into().unwrap(),
            'bad model class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad model address',
        );
    } else {
        core::panic_with_felt252('no EventUpgraded event');
    }
}

#[test]
fn test_upgrade_event_with_member_changed() {
    let world = deploy_world_for_event_upgrades();

    drop_all_events(world.contract_address);

    world.upgrade_event("dojo", e_FooEventMemberChanged::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::EventUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Event::<FooEventMemberChanged>::selector(DOJO_NSH),
            'bad event selector',
        );
        assert(
            event.class_hash == e_FooEventMemberChanged::TEST_CLASS_HASH.try_into().unwrap(),
            'bad event class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad event address',
        );
    } else {
        core::panic_with_felt252('no EventUpgraded event');
    }
}

#[test]
#[should_panic(
    expected: (
        "Invalid new layout to upgrade the resource `dojo-FooEventBadLayoutType`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_event_with_bad_layout_type() {
    let world = deploy_world_for_event_upgrades();
    world.upgrade_event("dojo", e_FooEventBadLayoutType::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooEventMemberRemoved`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_event_with_member_removed() {
    let world = deploy_world_for_event_upgrades();
    world.upgrade_event("dojo", e_FooEventMemberRemoved::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooEventMemberAddedButRemoved`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_event_with_member_added_but_removed() {
    let world = deploy_world_for_event_upgrades();
    world
        .upgrade_event(
            "dojo", e_FooEventMemberAddedButRemoved::TEST_CLASS_HASH.try_into().unwrap(),
        );
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooEventMemberAddedButMoved`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_event_with_member_moved() {
    let world = deploy_world_for_event_upgrades();
    world.upgrade_event("dojo", e_FooEventMemberAddedButMoved::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooEventMemberIllegalChange`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_event_with_member_illegal_change() {
    let world = deploy_world_for_event_upgrades();
    world.upgrade_event("dojo", e_FooEventMemberIllegalChange::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on event (or its namespace) `FooEventMemberAdded`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_event_from_event_writer() {
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world_for_event_upgrades();

    world.grant_writer(Event::<FooEventMemberAdded>::selector(DOJO_NSH), alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.upgrade_event("dojo", e_FooEventMemberAdded::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: ("Resource (Event) `dojo-SimpleEvent` is already registered", 'ENTRYPOINT_FAILED'),
)]
fn test_upgrade_event_from_random_account() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);
    world.grant_owner(DOJO_NSH, alice);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_event("dojo", e_SimpleEvent::TEST_CLASS_HASH.try_into().unwrap());

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.register_event("dojo", e_SimpleEvent::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Namespace `another_namespace` is not registered", 'ENTRYPOINT_FAILED'))]
fn test_register_event_with_unregistered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_event("another_namespace", e_SimpleEvent::TEST_CLASS_HASH.try_into().unwrap());
}

// It's CONTRACT_NOT_DEPLOYED for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the register_event function.
#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED'))]
fn test_register_event_through_malicious_contract() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);
    world.register_event("dojo", e_SimpleEvent::TEST_CLASS_HASH.try_into().unwrap());
}
