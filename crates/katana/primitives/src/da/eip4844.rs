use std::str::FromStr;

use lazy_static::lazy_static;
use num_bigint::{BigUint, ToBigUint};

// ****************************************************************************
// * PARAMETERS & CONSTANTS                                                  *
// ****************************************************************************
/// Length of the blob.
pub const BLOB_LEN: usize = 4096;

lazy_static! {
    /// EIP-4844 BLS12-381 modulus.
    ///
    /// As defined in https://eips.ethereum.org/EIPS/eip-4844
    pub static ref BLS_MODULUS: BigUint = BigUint::from_str(
        "52435875175126190479447740508185965837690552500527637822603658699938581184513",
    )
    .unwrap();
    /// Generator of the group of evaluation points (EIP-4844 parameter).
    pub static ref GENERATOR: BigUint = BigUint::from_str(
        "39033254847818212395286706435128746857159659164139250548781411570340225835782",
    )
    .unwrap();
    pub static ref TWO: BigUint = 2u32.to_biguint().unwrap();
}
