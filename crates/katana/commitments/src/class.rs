use bitvec::{slice::BitSlice, prelude::Msb0};
use katana_merkle_tree::{hash::FeltHash, merkle_tree::{MerkleTree, ProofNode}};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet_crypto::{poseidon_hash, FieldElement};

pub struct ClassTree<H: FeltHash> {
    root: FieldElement,
    tree: MerkleTree<H>,
}

// A class tree is a Merkle Patricia tree that stores the class state.
// The class state is the hash of the class code and the class storage root.
// The class state is stored in the leafs of the tree.
// The root of the tree is the hash of the class state.

impl <H: FeltHash> ClassTree<H> {
    pub fn new (root: FieldElement, tree: MerkleTree<H>) -> Self {
        Self { root, tree }
    }

    pub fn calculate_class(&mut self, compiled_class_hash: FieldElement) -> FieldElement {
        let class_state = H::hash(cairo_short_string_to_felt("CONTRACT_CLASS_LEAF_V0").unwrap(), compiled_class_hash);
        class_state
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
    use katana_merkle_tree::hash::PedersenHasher;
    use bitvec::{bitvec, prelude::Msb0};
    use super::*;

    //Test to calculate the class state and commit to the result to the tree.
    #[test]
    fn test_commitment_tree() {
        let tree = MerkleTree::<PedersenHasher>::new(FieldElement::ZERO);
        let compiled_class_hash = "0x001".parse::<FieldElement>().unwrap()   ;
        let mut class_tree = ClassTree::new(FieldElement::ZERO, tree);
        let result = class_tree.calculate_class(compiled_class_hash);
        let key = bitvec![u8,Msb0; 0, 0, 0, 0, 0, 0, 0, 0];
        class_tree.set(&key, result);
        class_tree.commit();
        let root = class_tree.root();
        println!("root: {:?}", root);
        assert_eq!(FieldElement::from_hex_be("0x07d370040a5bce2ebc29cf0a0f504cc06e86c2c7495bece4c303ff4c42980d3b").unwrap(), root);
    }
}
