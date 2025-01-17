use core::starknet::ContractAddress;

use dojo::world::IWorldDispatcher;

use crate::world::{spawn_test_world, NamespaceDef, TestResource};

/// This file contains some partial model contracts written without the dojo::model
/// attribute, to avoid having several contracts with a same name/classhash,
/// as the test runner does not differenciate them.
/// These model contracts are used to test model upgrades in tests/model.cairo.

#[derive(IntrospectPacked, Copy, Drop, Serde)]
#[dojo::model]
struct FooModelBadLayoutType {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
struct FooModelMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
struct FooModelMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
struct FooModelMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
struct FooModelMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}


pub fn deploy_world_for_model_upgrades() -> IWorldDispatcher {
    let namespace_def = NamespaceDef {
        namespace: "dojo",
        resources: [
            TestResource::Model(m_FooModelBadLayoutType::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(m_FooModelMemberRemoved::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(
                m_FooModelMemberAddedButRemoved::TEST_CLASS_HASH.try_into().unwrap(),
            ),
            TestResource::Model(m_FooModelMemberAddedButMoved::TEST_CLASS_HASH.try_into().unwrap()),
            TestResource::Model(m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap()),
        ]
            .span(),
    };
    spawn_test_world([namespace_def].span()).dispatcher
}
