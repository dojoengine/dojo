use alloy_primitives::U256;

use crate::FieldElement;

pub mod class;
pub mod transaction;

/// Split a [U256] into its high and low 128-bit parts in represented as [FieldElement]s.
/// The first element in the returned tuple is the low part, and the second element is the high
/// part.
pub fn split_u256(value: U256) -> (FieldElement, FieldElement) {
    let low_u128: u128 = value.to::<u128>();
    let high_u128: u128 = U256::from(value >> 128).to::<u128>();
    (FieldElement::from(low_u128), FieldElement::from(high_u128))
}
