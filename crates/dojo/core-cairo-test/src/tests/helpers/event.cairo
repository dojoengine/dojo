use core::starknet::ContractAddress;

use dojo::world::{IWorldDispatcher};

use crate::world::{spawn_test_world, NamespaceDef, TestResource};

/// This file contains some partial event contracts written without the dojo::event
/// attribute, to avoid having several contracts with a same name/classhash,
/// as the test runner does not differenciate them.
/// These event contracts are used to test event upgrades in tests/event.cairo.

// This event is used as a base to create the "previous" version of an event to be upgraded.
#[derive(Introspect)]
struct FooBaseEvent {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[dojo::event]
pub struct FooEventBadLayoutType {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[dojo::event]
pub struct FooEventMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
}

#[dojo::event]
pub struct FooEventMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub c: u256,
    pub d: u256
}

#[dojo::event]
pub struct FooEventMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub a: felt252,
    pub c: u256
}

#[dojo::event]
pub struct FooEventMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
    pub c: u256
}

pub fn deploy_world_for_event_upgrades() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo", resources: [
            TestResource::Event(old_foo_event_bad_layout_type::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Event(old_foo_event_member_removed::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Event(
                old_foo_event_member_added_but_removed::TEST_CLASS_HASH.try_into().unwrap()
            ),
            TestResource::Event(
                old_foo_event_member_added_but_moved::TEST_CLASS_HASH.try_into().unwrap()
            ),
            TestResource::Event(old_foo_event_member_added::TEST_CLASS_HASH.try_into().unwrap()),
        ].span()
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

#[starknet::contract]
pub mod old_foo_event_member_removed {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberRemoved"
        }
    }
    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventMemberRemoved';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseEvent>::layout()
        }
    }
}

#[starknet::contract]
pub mod old_foo_event_member_added_but_removed {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberAddedButRemoved"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventMemberAddedButRemoved';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseEvent>::layout()
        }
    }
}

#[starknet::contract]
pub mod old_foo_event_member_added_but_moved {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberAddedButMoved"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventMemberAddedButMoved';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseEvent>::layout()
        }
    }
}

#[starknet::contract]
pub mod old_foo_event_member_added {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberAdded"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventMemberAdded';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseEvent>::layout()
        }
    }
}
