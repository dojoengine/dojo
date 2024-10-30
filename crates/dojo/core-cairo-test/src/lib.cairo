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

    mod helpers;

    mod world {
        mod acl;
        //mod entities;
    //mod resources;
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
