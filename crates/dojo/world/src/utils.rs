//! Utility functions for the world.

use starknet::core::types::Felt;
use starknet::core::utils::{self as snutils, CairoShortStringToFeltError};
use starknet_crypto::poseidon_hash_single;

/// Computes the deterministic address of the world contract based on the given seed.
pub fn compute_world_address(
    seed: &str,
    world_class_hash: Felt,
) -> Result<Felt, CairoShortStringToFeltError> {
    let seed_felt = snutils::cairo_short_string_to_felt(seed)?;
    let salt = poseidon_hash_single(seed_felt);

    Ok(snutils::get_contract_address(salt, world_class_hash, &[world_class_hash], Felt::ZERO))
}

/// Computes the deterministic address of a Dojo contract based on the given selector, class hash
/// and world address.
pub fn compute_dojo_contract_address(
    dojo_selector: Felt,
    class_hash: Felt,
    world_address: Felt,
) -> Felt {
    snutils::get_contract_address(dojo_selector, class_hash, &[], world_address)
}
