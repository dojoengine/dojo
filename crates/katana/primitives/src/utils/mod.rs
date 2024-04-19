use alloy_primitives::U256;

use crate::FieldElement;

pub mod class;
pub mod transaction;

/// Split a [U256] into its high and low 128-bit parts in represented as [FieldElement]s.
/// The first element in the returned tuple is the low part, and the second element is the high
/// part.
pub fn split_u256(value: U256) -> (FieldElement, FieldElement) {
    let low_u128: u128 = (value & U256::from(u128::MAX)).to();
    let high_u128: u128 = U256::from(value >> 128).to();
    (FieldElement::from(low_u128), FieldElement::from(high_u128))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_u256() {
        // Given
        let value = U256::MAX;

        // When
        let (low, high) = split_u256(value);

        // Then
        assert_eq!(low, FieldElement::from(u128::MAX));
        assert_eq!(high, FieldElement::from(u128::MAX));
    }
}
