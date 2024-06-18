//! Felt conversion.
//!
//! Starknet-rs should normally migrate to starknet types core.
//! To be removed once it's ok as the CairoVM is already using
//! the core types.
use starknet::core::types::FieldElement;
use starknet_types_core::felt::Felt;

/// Converts a starknet-rs [`FieldElement`] to a starknet types core [`Felt`].
///
/// # Arguments
///
/// * `ff` - Starknet-rs [`FieldElement`].
pub fn from_ff(ff: &FieldElement) -> Felt {
    Felt::from_bytes_be(&ff.to_bytes_be())
}

/// Converts a vec of [`FieldElement`] to a vec of starknet types core [`Felt`].
///
/// # Arguments
///
/// * `ffs` - Starknet-rs [`&[FieldElement]`].
pub fn from_ff_vec(ffs: &[FieldElement]) -> Vec<Felt> {
    ffs.iter().map(from_ff).collect()
}
