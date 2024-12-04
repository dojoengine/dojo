#[cfg(test)]
mod utils;
#[cfg(test)]
mod snf_utils;
#[cfg(test)]
mod world;

#[cfg(test)]
pub use utils::{GasCounter, assert_array, GasCounterTrait};
#[cfg(test)]
pub use world::{
    spawn_test_world, NamespaceDef, TestResource, ContractDef, ContractDefTrait,
    WorldStorageTestTrait,
};

#[cfg(test)]
mod tests {
    mod meta {
        mod introspect;
    }

    mod event {
        mod event;
    }

    mod model {
        mod model;
    }

    mod storage {
        mod database;
        mod packing;
        mod storage;
    }

    // mod benchmarks;

    mod expanded {
        pub(crate) mod selector_attack;
    }

    mod helpers {
        mod helpers;
        pub use helpers::{
            DOJO_NSH, SimpleEvent, e_SimpleEvent, Foo, m_Foo, m_FooInvalidName, foo_setter,
            test_contract, test_contract_with_dojo_init_args, Sword, Case, Character, Abilities,
            Stats, Weapon, Ibar, IbarDispatcher, IbarDispatcherTrait, bar, deploy_world,
            deploy_world_and_bar, deploy_world_and_foo, IFooSetter, IFooSetterDispatcher,
            IFooSetterDispatcherTrait, NotCopiable, malicious_contract
        };

        mod event;
        pub use event::deploy_world_for_event_upgrades;

        mod model;
        pub use model::deploy_world_for_model_upgrades;
    }

    mod world {
        mod acl;
        mod contract;
        //mod entities;
        mod event;
        mod metadata;
        mod model;
        mod namespace;
        mod storage;
        mod world;
    }

    mod utils {
        mod hash;
        mod key;
        mod layout;
        mod misc;
        mod naming;
    }
}
