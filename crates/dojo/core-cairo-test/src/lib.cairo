//! Testing library for Dojo using Cairo test runner.

#[cfg(target: "test")]
mod utils;
#[cfg(target: "test")]
mod world;

#[cfg(target: "test")]
pub use utils::{GasCounter, assert_array, GasCounterTrait};
#[cfg(target: "test")]
pub use world::{
    deploy_contract, deploy_with_world_address, spawn_test_world, NamespaceDef, TestResource,
    ContractDef, ContractDefTrait
};

#[cfg(test)]
mod tests {
    mod meta {
        mod introspect;
    }

    mod event {
        mod event;
    }

    // mod model {
    //     mod model;
    // }

    mod storage {
        mod database;
        mod packing;
        mod storage;
    }

    mod contract;
    // mod benchmarks;

    mod expanded {
        pub(crate) mod selector_attack;
    }

    mod helpers {
        mod helpers;
        pub use helpers::{
            DOJO_NSH, SimpleEvent, e_SimpleEvent, Foo, m_Foo, foo_invalid_name, foo_setter,
            test_contract, test_contract_with_dojo_init_args, Sword, Case, Character, Abilities,
            Stats, Weapon, Ibar, bar, deploy_world, deploy_world_and_bar, deploy_world_and_foo,
            drop_all_events, IFooSetter, IFooSetterDispatcher, IFooSetterDispatcherTrait
        };

        mod event;
        pub use event::{
            FooEventBadLayoutType, e_FooEventBadLayoutType, FooEventMemberRemoved,
            e_FooEventMemberRemoved, FooEventMemberAddedButRemoved, e_FooEventMemberAddedButRemoved,
            FooEventMemberAddedButMoved, e_FooEventMemberAddedButMoved, FooEventMemberAdded,
            e_FooEventMemberAdded, deploy_world_for_event_upgrades
        };

        mod model;
        pub use model::{
            SimpleModel, m_SimpleModel, FooModelBadLayoutType, m_FooModelBadLayoutType,
            FooModelMemberRemoved, m_FooModelMemberRemoved, FooModelMemberAddedButRemoved,
            m_FooModelMemberAddedButRemoved, FooModelMemberAddedButMoved,
            m_FooModelMemberAddedButMoved, FooModelMemberAdded, m_FooModelMemberAdded,
            deploy_world_for_model_upgrades
        };
    }

    mod world {
        mod acl;
        mod contract;
        //mod entities;
        mod event;
        mod metadata;
        mod model;
        mod namespace;
        //mod world;
    }

    mod utils {
        mod hash;
        mod key;
        mod layout;
        mod misc;
        mod naming;
    }
}
