use bitvec::{slice::BitSlice, prelude::Msb0};
use starknet_crypto::{FieldElement, poseidon_hash_many};
use starknet::core::utils::cairo_short_string_to_felt;
use katana_merkle_tree::{merkle_tree::MerkleTree, hash::FeltHash};


pub struct StateCommitmentTree<H: FeltHash> {
    root: FieldElement,
    tree: MerkleTree<H>,
}

// A state commitment tree is a Merkle Patricia tree that stores the state commitment.
// The state commitment is the hash of the contract state and the class state.
// The state commitment is stored in the leafs of the tree.
impl <H: FeltHash> StateCommitmentTree<H> {
    pub fn new (root: FieldElement, tree: MerkleTree<H>) -> Self {
        Self { root, tree }
    }

    pub fn calculate_state_commitment(&mut self, contract_state_root: FieldElement, class_state_root: FieldElement) -> FieldElement {
        let prefix = cairo_short_string_to_felt("STARKNET_STATE_V0").unwrap();
        let state_commitment = H::multipleHash(&[prefix, contract_state_root, class_state_root]);
        state_commitment
    }

    pub fn commit(&mut self) {
        self.tree.commit();
    }

    pub fn root(&self) -> FieldElement {
        self.tree.root()
    }

    pub fn set(&mut self, key: &BitSlice<u8, Msb0>, value: FieldElement) {
        self.tree.set(key, value);
    }
}

#[cfg(test)]

mod test {
    use katana_merkle_tree::hash::PoseidonHasher;
    use bitvec::{bitvec, prelude::Msb0};
    use super::*;

    #[test]
    fn test_state_commitment_tree(){
        let tree = MerkleTree::<PoseidonHasher>::new(FieldElement::ZERO);
        let contract_state_root = "0x001".parse::<FieldElement>().unwrap()   ;
        let class_state_root = "0x001".parse::<FieldElement>().unwrap()   ;
        let mut state_commitment_tree = StateCommitmentTree::new(FieldElement::ZERO, tree);
        let result = state_commitment_tree.calculate_state_commitment(contract_state_root, class_state_root);
        let key = bitvec![u8,Msb0; 0, 0, 0, 0, 0, 0, 0, 0];
        state_commitment_tree.set(&key, result);
        state_commitment_tree.commit();
        let root = state_commitment_tree.root();
        println!("root: {:?}", root);
        assert_eq!(FieldElement::from_hex_be("0x010b0ff18b95cb87d2d11398050490384abccc5e4209f2824b6dc2276e66dfa2").unwrap(), root);
    }
}