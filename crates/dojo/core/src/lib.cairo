pub mod contract {
    pub mod interface;
    pub use interface::{IContract, IContractDispatcher, IContractDispatcherTrait};

    pub mod components {
        pub mod upgradeable;
        pub mod world_provider;
    }
}

pub mod event {
    pub mod event;
    pub use event::{Event, EventDefinition};

    pub mod interface;
    pub use interface::{IEvent, IEventDispatcher, IEventDispatcherTrait};

    #[cfg(target: "test")]
    pub use event::{EventTest};
}

pub mod meta {
    pub mod introspect;
    pub use introspect::{Introspect, Ty};

    pub mod layout;
    pub use layout::{Layout, FieldLayout};
}

pub mod model {
    pub mod component;

    pub mod definition;
    pub use definition::{ModelIndex, ModelDefinition, ModelDef};

    pub mod members;
    pub use members::{MemberStore};

    pub mod model;
    pub use model::{Model, ModelStore};

    pub mod model_value;
    pub use model_value::{ModelValue, ModelValueStore, ModelValueKey};

    pub mod interface;
    pub use interface::{IModel, IModelDispatcher, IModelDispatcherTrait};

    pub mod metadata;
    pub use metadata::ResourceMetadata;

    pub mod storage;
    pub use storage::{
        ModelStorage, MemberModelStorage, ModelStorageTest, ModelValueStorage, ModelValueStorageTest
    };

    #[cfg(target: "test")]
    pub use model::{ModelTest};

    #[cfg(target: "test")]
    pub use model_value::{ModelValueTest};
}

pub(crate) mod storage {
    pub(crate) mod database;
    pub(crate) mod packing;
    pub(crate) mod layout;
    pub(crate) mod storage;
    pub(crate) mod entity_model;
}

pub mod utils {
    // Since Scarb 2.6.0 there's an optimization that does not
    // build tests for dependencies and it's not configurable.
    //
    // To expose correctly the test utils for a package using dojo-core,
    // we need to it in the `lib` target or using the `#[cfg(target: "test")]`
    // attribute.
    //
    // Since `test_utils` is using `TEST_CLASS_HASH` to factorize some deployment
    // core, we place it under the test target manually.
    #[cfg(target: "test")]
    pub mod test;

    // #[cfg(target: "test")]
    // pub mod snf_test;

    pub mod descriptor;
    pub use descriptor::{
        Descriptor, DescriptorTrait, IDescriptorDispatcher, IDescriptorDispatcherTrait
    };

    pub mod hash;
    pub use hash::{bytearray_hash, selector_from_names, selector_from_namespace_and_name};

    pub mod key;
    pub use key::{entity_id_from_keys, combine_key, entity_id_from_key};

    pub mod layout;
    pub use layout::{find_field_layout, find_model_field_layout};

    pub mod misc;
    pub use misc::{any_none, sum};

    pub mod naming;
    pub use naming::is_name_valid;

    pub mod serde;
    pub use serde::{serialize_inline, deserialize_unwrap};
}

pub mod world {
    pub(crate) mod errors;

    mod resource;
    pub use resource::{Resource, ResourceIsNoneTrait};

    mod iworld;
    pub use iworld::{
        IWorld, IWorldDispatcher, IWorldDispatcherTrait, IUpgradeableWorld,
        IUpgradeableWorldDispatcher, IUpgradeableWorldDispatcherTrait
    };

    #[cfg(target: "test")]
    pub use iworld::{IWorldTest, IWorldTestDispatcher, IWorldTestDispatcherTrait};

    mod world_contract;
    pub use world_contract::world;

    pub mod storage;
    pub use storage::{WorldStorage, WorldStorageTrait};
}

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
    mod contract;
    mod benchmarks;
    mod expanded {
        pub(crate) mod selector_attack;
    }
    mod helpers;
    mod world {
        mod acl;
        mod entities;
        mod resources;
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
