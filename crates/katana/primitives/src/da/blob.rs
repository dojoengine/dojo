use num_bigint::BigUint;
use num_traits::Num;

use super::eip4844::{BLOB_LEN, BLS_MODULUS, GENERATOR};
use super::math::{fft, ifft};

/// Recovers the original data from a given blob.
///
/// This function takes a vector of `BigUint` representing the data of a blob and
/// returns the recovered original data as a vector of `BigUint`.
///
/// # Arguments
///
/// * `data` - A vector of `BigUint` representing the blob data.
///
/// # Returns
///
/// A vector of `BigUint` representing the recovered original data.
pub fn recover(data: Vec<BigUint>) -> Vec<BigUint> {
    let xs: Vec<BigUint> = (0..BLOB_LEN)
        .map(|i| {
            let bin = format!("{:012b}", i);
            let bin_rev = bin.chars().rev().collect::<String>();
            GENERATOR.modpow(&BigUint::from_str_radix(&bin_rev, 2).unwrap(), &BLS_MODULUS)
        })
        .collect();

    ifft(data, xs, &BLS_MODULUS)
}

pub fn transform(data: Vec<BigUint>) -> Vec<BigUint> {
    let xs: Vec<BigUint> = (0..BLOB_LEN)
        .map(|i| {
            let bin = format!("{:012b}", i);
            let bin_rev = bin.chars().rev().collect::<String>();
            GENERATOR.modpow(&BigUint::from_str_radix(&bin_rev, 2).unwrap(), &BLS_MODULUS)
        })
        .collect();

    fft(data, xs, &BLS_MODULUS)
}
