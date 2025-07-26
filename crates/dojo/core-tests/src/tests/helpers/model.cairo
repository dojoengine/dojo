use dojo::meta::introspect::Introspect;
use dojo::model::ModelIndex;
use dojo::utils::{entity_id_from_serialized_keys, selector_from_names, serialize_inline};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_snf_test::world::{NamespaceDef, TestResource, spawn_test_world};
use starknet::ContractAddress;
use super::helpers::{MyEnum, MyNestedEnum};

// This model is used as a base to create the "previous" version of a model to be upgraded.
#[derive(Introspect)]
struct FooBaseModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

// Old version of the FooModelMemberAdded model.
// Do not tag it as a dojo::model because it will lead to having 2 contracts with the same name.
#[derive(Introspect)]
pub struct FooModelMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

// Old version of the FooModelMemberChanged model.
// Do not tag it as a dojo::model because it will lead to having 2 contracts with the same name.
#[derive(Introspect)]
struct FooModelMemberChanged {
    #[key]
    pub caller: ContractAddress,
    pub a: (MyEnum, u8),
    pub b: u128,
}

pub fn old_model_with_nested_enum_key_layout() -> dojo::meta::layout::Layout {
    dojo::meta::layout::Layout::Struct(
        [
            dojo::meta::layout::FieldLayout {
                selector: selector!("a"), layout: dojo::meta::layout::Layout::Fixed([8].span()),
            },
        ]
            .span(),
    )
}

pub fn deploy_world_for_model_upgrades() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo",
        resources: [
            TestResource::Model("OldFooModelBadLayoutType"),
            TestResource::Model("OldFooModelMemberRemoved"),
            TestResource::Model("OldFooModelMemberAddedButRemoved"),
            TestResource::Model("OldFooModelMemberAddedButMoved"),
            TestResource::Model("OldFooModelMemberAdded"),
            TestResource::Model("OldFooModelMemberChanged"),
            TestResource::Model("OldFooModelMemberIllegalChange"),
            TestResource::Model("OldModelWithNestedEnumKey"),
        ]
            .span(),
    };
    let world = spawn_test_world([namespace_def].span()).dispatcher;

    // write some model values to be able to check if after a successfully upgrade, these values
    // remain the same

    // write FooModelMemberAdded { caller: 0xb0b, a: 123, b: 456 }
    world
        .set_entity(
            selector_from_names(@"dojo", @"FooModelMemberAdded"),
            ModelIndex::Keys([0xb0b].span()),
            [123, 456].span(),
            Introspect::<FooModelMemberAdded>::layout(),
        );

    // write FooModelMemberChanged { caller: 0xb0b, a: (MyEnum::X(42), 189), b: 456 }
    world
        .set_entity(
            selector_from_names(@"dojo", @"FooModelMemberChanged"),
            ModelIndex::Keys([0xb0b].span()),
            [1, 42, 189, 456].span(),
            Introspect::<FooModelMemberChanged>::layout(),
        );

    // write ModelWithNestedEnumKey { k: MyNestedEnum::A(MyEnum::X(8)), a: 42 }

    let keys = serialize_inline(@MyNestedEnum::A(MyEnum::X(8)));
    world
        .set_entity(
            selector_from_names(@"dojo", @"ModelWithNestedEnumKey"),
            ModelIndex::Id(entity_id_from_serialized_keys(keys)),
            [42].span(),
            old_model_with_nested_enum_key_layout(),
        );

    world
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


#[starknet::contract]
pub mod m_OldFooModelMemberChanged {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooModelMemberChanged"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooModelMemberChanged>::ty() {
                s.name = 'FooModelMemberChanged';
                s
            } else {
                panic!("Unexpected schema.")
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::introspect::Introspect::<super::FooModelMemberChanged>::layout()
        }
    }
}


#[starknet::contract]
pub mod m_OldFooModelMemberIllegalChange {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "FooModelMemberIllegalChange"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            if let dojo::meta::introspect::Ty::Struct(mut s) =
                dojo::meta::introspect::Introspect::<super::FooBaseModel>::ty() {
                s.name = 'FooModelMemberIllegalChange';
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
pub mod m_OldModelWithNestedEnumKey {
    // In this old version of ModelWithNestedEnumKey, the type of the key member `k` is:
    // ```
    // enum MyNestedEnum {
    //     A: MyEnum,
    //     B: u16
    // }
    //
    // enum MyEnum {
    //     X: u8,
    // }
    // ```

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DeployedModelImpl of dojo::meta::interface::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "ModelWithNestedEnumKey"
        }
    }

    #[abi(embed_v0)]
    impl StoredImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            dojo::meta::introspect::Struct {
                name: 'ModelWithNestedEnumKey',
                attrs: [].span(),
                children: [
                    dojo::meta::introspect::Member {
                        name: 'k',
                        attrs: ['key'].span(),
                        ty: dojo::meta::introspect::Ty::Enum(
                            dojo::meta::introspect::Enum {
                                name: 'MyNestedEnum',
                                attrs: [].span(),
                                children: [
                                    (
                                        'A',
                                        dojo::meta::introspect::Ty::Enum(
                                            dojo::meta::introspect::Enum {
                                                name: 'MyEnum',
                                                attrs: [].span(),
                                                children: [
                                                    (
                                                        'X',
                                                        dojo::meta::introspect::Ty::Primitive('u8'),
                                                    )
                                                ]
                                                    .span(),
                                            },
                                        ),
                                    ),
                                    ('B', dojo::meta::introspect::Ty::Primitive('u16')),
                                ]
                                    .span(),
                            },
                        ),
                    },
                    dojo::meta::introspect::Member {
                        name: 'a',
                        attrs: [].span(),
                        ty: dojo::meta::introspect::Ty::Primitive('u8'),
                    },
                ]
                    .span(),
            }
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            super::old_model_with_nested_enum_key_layout()
        }
    }
}
