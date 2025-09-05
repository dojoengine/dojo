use dojo::world::IWorldDispatcher;
use dojo_snf_test::world::{NamespaceDef, TestResource, spawn_test_world};
use starknet::ContractAddress;
use super::helpers::MyEnum;

// This event is used as a base to create the "previous" version of an event to be upgraded.
#[derive(Introspect, Serde)]
struct FooBaseEvent {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

// To get the schema and layout for the old version of the FooEventMemberChanged event.
#[derive(Introspect)]
struct OldFooEventMemberChanged {
    #[key]
    pub caller: ContractAddress,
    pub a: (MyEnum, u8),
    pub b: u128,
}

pub fn deploy_world_for_event_upgrades() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo",
        resources: [
            TestResource::Event("OldFooEventBadLayoutType"),
            TestResource::Event("OldFooEventMemberRemoved"),
            TestResource::Event("OldFooEventMemberAddedButRmd"),
            TestResource::Event("OldFooEventMemberAddedButMvd"),
            TestResource::Event("OldFooEventMemberAdded"),
            TestResource::Event("OldFooEventMemberChanged"),
            TestResource::Event("OldFooEventMemberBadChange"),
        ]
            .span(),
    };
    spawn_test_world([namespace_def].span()).dispatcher
}

/// This file contains some partial event contracts written without the dojo::event
/// attribute, to avoid having several contracts with a same name,
/// as the snfoundry test runner does not differenciate them.
/// These event contracts are used to test event upgrades in tests/event.cairo.

#[starknet::contract]
pub mod e_OldFooEventBadLayoutType {
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
pub mod e_OldFooEventMemberRemoved {
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
pub mod e_OldFooEventMemberAddedButRmd {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberAddedButRmd"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventMemberAddedButRmd';
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
pub mod e_OldFooEventMemberAddedButMvd {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberAddedButMvd"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventMemberAddedButMvd';
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
pub mod e_OldFooEventMemberAdded {
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

#[starknet::contract]
pub mod e_OldFooEventMemberChanged {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberChanged"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::OldFooEventMemberChanged>::ty() {
                s.name = 'FooEventMemberChanged';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::OldFooEventMemberChanged>::layout()
        }
    }
}

#[starknet::contract]
pub mod e_OldFooEventMemberBadChange {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedEventImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooEventMemberBadChange"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseEvent>::ty() {
                s.name = 'FooEventMemberBadChange';
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
