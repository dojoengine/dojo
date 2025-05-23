#[cfg(test)]
mod utils;

#[cfg(test)]
mod tests {
    mod contract;

    mod event {
        mod event;
    }

    mod expanded {
        pub(crate) mod bytearray_hash;
        pub(crate) mod selector_attack;
    }

    mod helpers {
        mod helpers;
        pub use helpers::{
            Abilities, Case, Character, DOJO_NSH, EnumOne, Foo, IFooSetter, IFooSetterDispatcher,
            IFooSetterDispatcherTrait, Ibar, IbarDispatcher, IbarDispatcherTrait, MyEnum,
            NotCopiable, SimpleEvent, Stats, Sword, Weapon, WithOptionAndEnums, bar, deploy_world,
            deploy_world_and_bar, deploy_world_and_foo, deploy_world_with_all_kind_of_resources,
            e_SimpleEvent, foo_setter, m_Foo, m_FooInvalidName, test_contract,
            test_contract_with_dojo_init_args,
        };

        mod event;
        pub use event::deploy_world_for_event_upgrades;

        mod model;
        pub use model::deploy_world_for_model_upgrades;

        mod library;
        pub use library::*;
    }

    mod meta {
        mod introspect;
    }

    mod model {
        mod model;
    }

    mod storage {
        mod database;
        mod packing;
        mod storage;
    }

    mod utils {
        mod hash;
        mod key;
        mod layout;
        mod misc;
        mod naming;
    }

    mod world {
        mod acl;
        mod contract;
        mod event;
        mod metadata;
        mod model;
        mod namespace;
        mod storage;
        mod world;
    }
}
