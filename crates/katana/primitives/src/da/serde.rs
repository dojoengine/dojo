use num_bigint::BigUint;
use num_traits::Num;

use super::eip4844::BLOB_LEN;

/// Parse a string and return a vector of `BigUint` representing the data.
/// # Arguments
/// * `data` - The string to parse.
/// # Returns
/// A vector of `BigUint` representing the data.
pub fn parse_str_to_blob_data(data: &str) -> Vec<BigUint> {
    let blob_hex = data.trim();
    (0..BLOB_LEN)
        .map(|i| BigUint::from_str_radix(&blob_hex[i * 64..(i + 1) * 64], 16).expect("Invalid hex"))
        .collect()
}
