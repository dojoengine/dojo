pub mod contract {
    mod base_contract;
    pub use base_contract::base;
    pub mod contract;
    pub use contract::{IContract, IContractDispatcher, IContractDispatcherTrait};
    pub mod upgradeable;
}

pub mod model {
    pub mod introspect;
    pub mod layout;
    pub use layout::{Layout, FieldLayout};

    pub mod model;
    pub use model::{
        Model, ModelIndex, ModelEntity, IModel, IModelDispatcher, IModelDispatcherTrait,
        deploy_and_get_metadata
    };

    #[cfg(target: "test")]
    pub use model::{ModelTest, ModelEntityTest};

    pub mod metadata;
    pub use metadata::{ResourceMetadata, ResourceMetadataTrait, resource_metadata};
    pub(crate) use metadata::{initial_address, initial_class_hash};
}

pub(crate) mod storage {
    pub(crate) mod database;
    pub(crate) mod packing;
    pub(crate) mod layout;
    pub(crate) mod storage;
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

    pub mod utils;
    pub use utils::{
        bytearray_hash, entity_id_from_keys, find_field_layout, find_model_field_layout, any_none,
        sum, combine_key, selector_from_names
    };
}

pub mod world {
    pub(crate) mod update;
    pub(crate) mod config;
    pub(crate) mod errors;

    mod world_contract;
    pub use world_contract::{
        world, IWorld, IWorldDispatcher, IWorldDispatcherTrait, IWorldProvider,
        IWorldProviderDispatcher, IWorldProviderDispatcherTrait, Resource,
    };
    pub(crate) use world_contract::{
        IUpgradeableWorld, IUpgradeableWorldDispatcher, IUpgradeableWorldDispatcherTrait
    };

    #[cfg(target: "test")]
    pub use world_contract::{IWorldTest, IWorldTestDispatcher, IWorldTestDispatcherTrait};
}

#[cfg(test)]
mod tests {
    mod model {
        mod introspect;
        mod model;
    }
    mod storage {
        mod database;
        mod packing;
        mod storage;
    }
    mod base;
    mod benchmarks;
    mod helpers;
    mod world {
        mod acl;
        mod entities;
        mod resources;
        mod world;
    }
    mod utils;
}
