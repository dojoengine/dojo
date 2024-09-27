use std::ops::{Add, AddAssign, Sub, SubAssign};

use starknet::core::types::U256;
use starknet_crypto::Felt;

use super::FELT_DELIMITER;

pub fn felts_to_sql_string(felts: &[Felt]) -> String {
    felts.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(FELT_DELIMITER)
        + FELT_DELIMITER
}

pub fn felt_to_sql_string(felt: &Felt) -> String {
    format!("{:#x}", felt)
}

pub fn felt_and_u256_to_sql_string(felt: &Felt, u256: &U256) -> String {
    format!("{}:{}", felt_to_sql_string(felt), u256_to_sql_string(u256))
}

pub fn u256_to_sql_string(u256: &U256) -> String {
    format!("{:#064x}", u256)
}

pub fn sql_string_to_u256(sql_string: &str) -> U256 {
    let sql_string = sql_string.strip_prefix("0x").unwrap_or(sql_string);
    U256::from(crypto_bigint::U256::from_be_hex(sql_string))
}

// type used to do calculation on inmemory balances
#[derive(Debug, Clone, Copy)]
pub struct I256 {
    pub value: U256,
    pub is_negative: bool,
}

impl Default for I256 {
    fn default() -> Self {
        Self { value: U256::from(0u8), is_negative: false }
    }
}

impl From<U256> for I256 {
    fn from(value: U256) -> Self {
        Self { value, is_negative: false }
    }
}

impl From<u8> for I256 {
    fn from(value: u8) -> Self {
        Self { value: U256::from(value), is_negative: false }
    }
}

impl Add for I256 {
    type Output = I256;

    fn add(self, other: I256) -> I256 {
        if self.is_negative == other.is_negative {
            // Same sign: add the values and keep the sign
            I256 { value: self.value + other.value, is_negative: self.is_negative }
        } else {
            // Different signs: subtract the smaller value from the larger one
            if self.value >= other.value {
                I256 { value: self.value - other.value, is_negative: self.is_negative }
            } else {
                I256 { value: other.value - self.value, is_negative: other.is_negative }
            }
        }
    }
}

impl Sub for I256 {
    type Output = I256;

    fn sub(self, other: I256) -> I256 {
        if self.is_negative != other.is_negative {
            // Different signs: add the values and keep the sign of self
            I256 { value: self.value + other.value, is_negative: self.is_negative }
        } else {
            // Same sign: subtract the values
            if self.value >= other.value {
                I256 { value: self.value - other.value, is_negative: self.is_negative }
            } else {
                I256 { value: other.value - self.value, is_negative: !other.is_negative }
            }
        }
    }
}

impl AddAssign for I256 {
    fn add_assign(&mut self, other: I256) {
        *self = *self + other;
    }
}

impl SubAssign for I256 {
    fn sub_assign(&mut self, other: I256) {
        *self = *self - other;
    }
}
