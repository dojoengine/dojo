use core::starknet::ContractAddress;

use dojo::world::IWorldDispatcher;

use crate::world::{spawn_test_world, NamespaceDef, TestResource};

// This model is used as a base to create the "previous" version of a model to be upgraded.
#[derive(Introspect)]
struct FooBaseModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

pub fn deploy_world_for_model_upgrades() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo", resources: [
            TestResource::Model("OldFooModelBadLayoutType"),
            TestResource::Model("OldFooModelMemberRemoved"),
            TestResource::Model("OldFooModelMemberAddedButRemoved"),
            TestResource::Model("OldFooModelMemberAddedButMoved"),
            TestResource::Model("OldFooModelMemberAdded"),
        ].span()
    };
    spawn_test_world([namespace_def].span()).dispatcher
}

/// This file contains some partial model contracts written without the dojo::model
/// attribute, to avoid having several contracts with a same name,
/// as the snfoundry test runner does not differenciate them.
/// These model contracts are used to test model upgrades in tests/model.cairo.

#[starknet::contract]
pub mod m_OldFooModelBadLayoutType {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooModelBadLayoutType"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseModel>::ty() {
                s.name = 'FooModelBadLayoutType';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            // Should never happen as dojo::model always derive Introspect.
            dojo::meta::Layout::Fixed([].span())
        }
    }
}

#[starknet::contract]
pub mod m_OldFooModelMemberRemoved {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooModelMemberRemoved"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseModel>::ty() {
                s.name = 'FooModelMemberRemoved';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseModel>::layout()
        }
    }
}

#[starknet::contract]
pub mod m_OldFooModelMemberAddedButRemoved {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooModelMemberAddedButRemoved"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseModel>::ty() {
                s.name = 'FooModelMemberAddedButRemoved';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseModel>::layout()
        }
    }
}

#[starknet::contract]
pub mod m_OldFooModelMemberAddedButMoved {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooModelMemberAddedButMoved"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseModel>::ty() {
                s.name = 'FooModelMemberAddedButMoved';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseModel>::layout()
        }
    }
}

#[starknet::contract]
pub mod m_OldFooModelMemberAdded {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooModelMemberAdded"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseModel>::ty() {
                s.name = 'FooModelMemberAdded';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooBaseModel>::layout()
        }
    }
}
