use std::array;

use starknet_crypto::{pedersen_hash, poseidon_hash, FieldElement, poseidon_hash_many};

pub struct PedersenHasher;

pub struct PoseidonHasher;

pub enum Hasher {
    Pedersen(PedersenHasher),
    Poseidon(PoseidonHasher),
}

pub trait FeltHash {
    fn hash(a: FieldElement, b: FieldElement) -> FieldElement;
    fn multipleHash(a: &[FieldElement]) -> FieldElement;
}

impl FeltHash for PedersenHasher {
    fn hash(a: FieldElement, b: FieldElement) -> FieldElement {
        pedersen_hash(&a, &b)
    }
    fn multipleHash(a: &[FieldElement]) -> FieldElement {
            match a.len() {
                0 => panic!("Cannot hash an empty slice"),
                1 => return a[0],
                2 => return pedersen_hash(&a[0], &a[1]),
                _ => {
                    if a.is_empty() {
                        panic!("Cannot hash an empty slice");
                    }
                
                    let mut current_hash = a[0].clone();
                    for i in 1..a.len() {
                        current_hash = pedersen_hash(&current_hash, &a[i]);
                    }
                    return current_hash;
                }
            }
    }
}

impl FeltHash for PoseidonHasher {
    fn hash(a: FieldElement, b: FieldElement) -> FieldElement {
        poseidon_hash(a, b)
    }

    fn multipleHash(a: &[FieldElement]) -> FieldElement {
        poseidon_hash_many(a)
    }
}
