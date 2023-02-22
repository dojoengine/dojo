use prisma_client_rust::bigdecimal::num_bigint::BigUint;
use sha3::{Digest, Keccak256};

pub fn starknet_hash(data: &[u8]) -> BigUint {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    // Truncate result to 250 bits.
    // *hash.first_mut().unwrap() &= 3;
    BigUint::from_bytes_be(&hash)
}
