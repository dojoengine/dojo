use starknet_crypto::{pedersen_hash, poseidon_hash, FieldElement};

pub struct PedersenHasher;

pub struct PoseidonHasher;

pub enum Hasher {
    Pedersen(PedersenHasher),
    Poseidon(PoseidonHasher),
}

pub trait FeltHash {
    fn hash(a: FieldElement, b: FieldElement) -> FieldElement;
}

impl FeltHash for PedersenHasher {
    fn hash(a: FieldElement, b: FieldElement) -> FieldElement {
        pedersen_hash(&a, &b)
    }
}

impl FeltHash for PoseidonHasher {
    fn hash(a: FieldElement, b: FieldElement) -> FieldElement {
        poseidon_hash(a, b)
    }
}
