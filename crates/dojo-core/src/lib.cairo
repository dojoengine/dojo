pub mod base;
#[cfg(test)]
mod base_test;
pub mod config;
pub mod database;
#[cfg(test)]
mod database_test;
pub mod interfaces;
pub mod model;
#[cfg(test)]
mod model_test;
pub mod contract;
pub mod packing;
#[cfg(test)]
mod packing_test;
pub mod world;
#[cfg(test)]
mod world_test;
pub mod utils;
#[cfg(test)]
mod utils_test;

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
pub mod test_utils;

#[cfg(test)]
mod benchmarks;

pub mod components;
pub mod resource_metadata;
