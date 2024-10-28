use core::starknet::ContractAddress;

use dojo::world::{IWorldDispatcher, WorldStorageTrait, WorldStorage};


use crate::world::{spawn_test_world, NamespaceDef, TestResource};

/// This file contains some partial model contracts written without the dojo::model
/// attribute, to avoid having several contracts with a same name/classhash,
/// as the test runner does not differenciate them.
/// These model contracts are used to test model upgrades in tests/model.cairo.

// This model is used as a base to create the "previous" version of a model to be upgraded.
#[derive(Introspect, Copy, Drop, Serde)]
struct FooBaseModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct SimpleModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

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

pub fn deploy_world_for_model_upgrades() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo", resources: [
            TestResource::Model(old_foo_model_bad_layout_type::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(old_foo_model_member_removed::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(
                old_foo_model_member_added_but_removed::TEST_CLASS_HASH.try_into().unwrap()
            ),
            TestResource::Model(
                old_foo_model_member_added_but_moved::TEST_CLASS_HASH.try_into().unwrap()
            ),
            TestResource::Model(old_foo_model_member_added::TEST_CLASS_HASH.try_into().unwrap()),
        ].span()
    };
    spawn_test_world([namespace_def].span()).dispatcher
}

#[starknet::contract]
pub mod old_foo_model_bad_layout_type {
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
            dojo::meta::Layout::Fixed([].span())
        }
    }
}

#[starknet::contract]
pub mod old_foo_model_member_removed {
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
pub mod old_foo_model_member_added_but_removed {
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
pub mod old_foo_model_member_added_but_moved {
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
pub mod old_foo_model_member_added {
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
