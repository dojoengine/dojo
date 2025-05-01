use core::starknet::ContractAddress;

use dojo::world::{IWorldDispatcher};

use crate::world::{spawn_test_world, NamespaceDef, TestResource};

/// This file contains some partial event contracts written without the dojo::event
/// attribute, to avoid having several contracts with a same name/classhash,
/// as the test runner does not differentiate them.
/// These event contracts are used to test event upgrades in tests/event.cairo.

// This event is used as a base to create the "previous" version of an event to be upgraded.
#[derive(Introspect, Serde)]
struct FooBaseEvent {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
pub struct FooEventBadLayoutType {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
struct FooEventMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
struct FooEventMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
struct FooEventMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Copy, Drop, Serde, Debug)]
#[dojo::event]
struct FooEventMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde, Default)]
enum MyEnum {
    #[default]
    X: u8,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::event]
struct FooEventMemberChanged {
    #[key]
    pub caller: ContractAddress,
    pub a: (MyEnum, u8),
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde, Default)]
enum AnotherEnum {
    #[default]
    X: u8,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::event]
struct FooEventMemberIllegalChange {
    #[key]
    pub caller: ContractAddress,
    pub a: AnotherEnum,
    pub b: u128,
}

pub fn deploy_world_for_event_upgrades() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo",
        resources: [
            TestResource::Event(old_foo_event_bad_layout_type::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Event(e_FooEventMemberRemoved::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Event(
                e_FooEventMemberAddedButRemoved::TEST_CLASS_HASH.try_into().unwrap(),
            ),
            TestResource::Event(e_FooEventMemberAddedButMoved::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Event(e_FooEventMemberAdded::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Event(e_FooEventMemberChanged::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Event(e_FooEventMemberIllegalChange::TEST_CLASS_HASH.try_into().unwrap()),
        ]
            .span(),
    };
    spawn_test_world([namespace_def].span()).dispatcher
}

#[starknet::contract]
pub mod old_foo_event_bad_layout_type {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventBadLayoutType"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventBadLayoutType';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            // Should never happen as dojo::event always derive Introspect.
            dojo::meta::Layout::Fixed([].span())
        }
    }
}
