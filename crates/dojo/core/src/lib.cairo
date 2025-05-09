pub mod contract {
    pub mod interface;
    pub use interface::{
        IContract, IContractDispatcher, IContractDispatcherTrait, ILibrary, ILibraryDispatcher,
        ILibraryDispatcherTrait,
    };

    pub mod components {
        pub mod upgradeable;
        pub mod world_provider;
    }
}

pub mod event {
    pub mod component;

    pub mod event;
    pub use event::{Event, EventDef, EventDefinition};

    pub mod interface;
    pub use interface::{IEvent, IEventDispatcher, IEventDispatcherTrait};

    pub mod storage;
    pub use storage::{EventStorage, EventStorageTest};
}

pub mod meta {
    pub mod interface;
    pub use interface::{
        IDeployedResource, IDeployedResourceDispatcher, IDeployedResourceDispatcherTrait,
        IDeployedResourceLibraryDispatcher, IStoredResource, IStoredResourceDispatcher,
        IStoredResourceDispatcherTrait,
    };

    pub mod introspect;
    pub use introspect::{Introspect, Ty, TyCompareTrait};

    pub mod layout;
    pub use layout::{FieldLayout, Layout, LayoutCompareTrait};
}

pub mod model {
    pub mod component;

    pub mod definition;
    pub use definition::{ModelDef, ModelDefinition, ModelIndex};

    pub mod model;
    pub use model::{KeyParser, Model, ModelPtr, ModelPtrsTrait};

    pub mod model_value;
    pub use model_value::{ModelValue, ModelValueKey};

    pub mod interface;
    pub use interface::{IModel, IModelDispatcher, IModelDispatcherTrait};

    pub mod metadata;
    pub use metadata::ResourceMetadata;

    pub mod storage;
    pub use storage::{ModelStorage, ModelStorageTest, ModelValueStorage, ModelValueStorageTest};
}

pub mod storage {
    pub mod database;
    pub mod entity_model;
    pub mod layout;
    pub mod packing;
    pub mod storage;
}

pub mod utils {
    pub mod hash;
    pub use hash::{
        bytearray_hash, selector_from_hashes, selector_from_names, selector_from_namespace_and_name,
    };

    pub mod key;
    pub use key::{combine_key, entity_id_from_keys, entity_id_from_serialized_keys};

    pub mod layout;
    pub use layout::{find_field_layout, find_model_field_layout};

    pub mod misc;
    pub use misc::{any_none, sum};

    pub mod naming;
    pub use naming::is_name_valid;

    pub mod serde;
    pub use serde::{deserialize_unwrap, serialize_inline};
}

pub mod world {
    pub(crate) mod errors;

    mod resource;
    pub use resource::{Resource, ResourceIsNoneTrait};

    mod iworld;
    pub use iworld::{
        IUpgradeableWorld, IUpgradeableWorldDispatcher, IUpgradeableWorldDispatcherTrait, IWorld,
        IWorldDispatcher, IWorldDispatcherTrait,
    };
    #[cfg(target: "test")]
    pub use iworld::{IWorldTest, IWorldTestDispatcher, IWorldTestDispatcherTrait};

    mod world_contract;
    pub use world_contract::world;

    pub mod storage;
    pub use storage::{WorldStorage, WorldStorageTrait};
}
