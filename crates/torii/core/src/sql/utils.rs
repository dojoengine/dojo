use starknet::core::types::U256;
use starknet_crypto::Felt;

use super::FELT_DELIMITER;

pub fn felts_sql_string(felts: &[Felt]) -> String {
    felts.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(FELT_DELIMITER)
        + FELT_DELIMITER
}

pub(crate) fn u256_to_sql_string(u256: &U256) -> String {
    format!("{:#064x}", u256)
}

pub(crate) fn sql_string_to_u256(sql_string: &str) -> U256 {
    let sql_string = sql_string.strip_prefix("0x").unwrap_or(sql_string);
    U256::from(crypto_bigint::U256::from_be_hex(sql_string))
}
