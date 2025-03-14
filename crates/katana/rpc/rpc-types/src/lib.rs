//! Types used in the Katana JSON-RPC API.
//!
//! Most of the types defined in this crate are simple wrappers around types imported from
//! `starknet-rs`.

pub mod account;
pub mod block;
pub mod class;
pub mod error;
pub mod event;
pub mod message;
pub mod receipt;
pub mod state_update;
pub mod trace;
pub mod transaction;
pub mod trie;

use std::ops::Deref;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use starknet::core::serde::unsigned_field_element::UfeHex;

/// A wrapper around [`FieldElement`](katana_primitives::FieldElement) that serializes to hex as
/// default.
#[serde_as]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeltAsHex(#[serde_as(serialize_as = "UfeHex")] katana_primitives::Felt);

impl From<katana_primitives::Felt> for FeltAsHex {
    fn from(value: katana_primitives::Felt) -> Self {
        Self(value)
    }
}

impl From<FeltAsHex> for katana_primitives::Felt {
    fn from(value: FeltAsHex) -> Self {
        value.0
    }
}

impl Deref for FeltAsHex {
    type Target = katana_primitives::Felt;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type FunctionCall = starknet::core::types::FunctionCall;

pub type FeeEstimate = starknet::core::types::FeeEstimate;

// pub type ContractClass = starknet::core::types::ContractClass;

pub type SimulationFlagForEstimateFee = starknet::core::types::SimulationFlagForEstimateFee;

pub type SimulationFlag = starknet::core::types::SimulationFlag;

pub type SyncingStatus = starknet::core::types::SyncStatusType;

#[cfg(test)]
mod tests {
    use serde_json::json;
    use starknet::macros::felt;

    use super::FeltAsHex;

    #[test]
    fn serde_felt() {
        let value = felt!("0x12345");
        let value_as_dec = json!(value);
        let value_as_hex = json!(format!("{value:#x}"));

        let expected_value = FeltAsHex(value);
        let actual_des_value: FeltAsHex = serde_json::from_value(value_as_dec).unwrap();
        assert_eq!(expected_value, actual_des_value, "should deserialize to decimal");

        let actual_ser_value = serde_json::to_value(expected_value).unwrap();
        assert_eq!(value_as_hex, actual_ser_value, "should serialize to hex");
    }
}
