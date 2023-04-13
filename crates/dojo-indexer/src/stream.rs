use apibara_core::starknet::v1alpha2::{Block, FieldElement, Filter};
use apibara_sdk::ClientBuilder;

/// Starknet version of [ClientBuilder].
pub type StarknetClientBuilder = ClientBuilder<Filter, Block>;

pub trait FieldElementExt {
    /// Returns the field element as [num::BigUint];
    fn to_biguint(&self) -> num::BigUint;

    /// Returns the field element as hex string, without the 0x prefix.
    fn to_hex_string(&self) -> String;
}

impl FieldElementExt for FieldElement {
    fn to_biguint(&self) -> num::BigUint {
        num::BigUint::from_bytes_be(&self.to_bytes())
    }

    fn to_hex_string(&self) -> String {
        hex::encode(self.to_bytes())
    }
}
