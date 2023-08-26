use bitvec::{slice::BitSlice, prelude::Msb0};
//Commitment tree for contract
use starknet_crypto::{FieldElement, pedersen_hash};
use katana_merkle_tree::{merkle_tree::MerkleTree, hash::FeltHash};


#[derive(Debug, Clone)]
pub struct ContractTree<H: FeltHash> {
    root: FieldElement,
    tree: MerkleTree<H>,
}

// A contract tree is a Merkle Patricia tree that stores the contract state.
// The contract state is the hash of the class hash, storage root and nonce.
// The contract state is stored in the leafs of the tree.
// The root of the tree is the hash of the contract state.

impl <H: FeltHash> ContractTree<H> {
    pub fn new (root: FieldElement, tree: MerkleTree<H>) -> Self {
        Self { root, tree }
    }

    pub fn calculate_contract(&mut self, class_hash: FieldElement, storage_root: FieldElement, nonce: FieldElement) -> FieldElement {
        let contract_state = H::multipleHash(&[class_hash, storage_root, nonce, FieldElement::ZERO]);
        contract_state
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

    //Test to calculate the contract state and commit to the result to the tree.
    #[test]
    fn test_commitment_tree() {
        let tree = MerkleTree::<PedersenHasher>::new(FieldElement::ZERO);
        let class_hash = "0x001".parse::<FieldElement>().unwrap()   ;
        let storage_root = "0x001".parse::<FieldElement>().unwrap()  ;
        let nonce = FieldElement::ONE;
        let mut contract_tree = ContractTree::new(FieldElement::ZERO, tree);
        let result = contract_tree.calculate_contract(class_hash, storage_root, nonce);
        let key = bitvec![u8,Msb0; 0, 0, 0, 0, 0, 0, 0, 0];
        contract_tree.set(&key, result);
        contract_tree.commit();
        let root = contract_tree.root();
        println!("root: {:?}", root);
        assert_eq!(FieldElement::from_hex_be("0x008c32ee51c4fea1a293a25dde7eaaad13688143a487a855d2b388f2892fbba5").unwrap(), root);
    }
}