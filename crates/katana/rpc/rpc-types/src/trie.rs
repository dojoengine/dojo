use std::ops::{Deref, DerefMut};

use katana_primitives::contract::StorageKey;
use katana_primitives::hash::StarkHash;
use katana_primitives::{ContractAddress, Felt};
use katana_trie::bitvec::view::BitView;
use katana_trie::{BitVec, MultiProof, Path, ProofNode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContractStorageKeys {
    #[serde(rename = "contract_address")]
    pub address: ContractAddress,
    #[serde(rename = "storage_keys")]
    pub keys: Vec<StorageKey>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalRoots {
    /// The associated block hash (needed in case the caller used a block tag for the block_id
    /// parameter).
    pub block_hash: Felt,
    pub classes_tree_root: Felt,
    pub contracts_tree_root: Felt,
}

/// Node in the Merkle-Patricia trie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MerkleNode {
    /// Represents a path to the highest non-zero descendant node.
    Edge {
        /// An integer whose binary representation represents the path from the current node to its
        /// highest non-zero descendant (bounded by 2^251)
        path: Felt,
        /// The length of the path (bounded by 251).
        length: u8,
        /// The hash of the unique non-zero maximal-height descendant node.
        child: Felt,
    },

    /// An internal node whose both children are non-zero.
    Binary {
        /// The hash of the left child.
        left: Felt,
        /// The hash of the right child.
        right: Felt,
    },
}

impl MerkleNode {
    // Taken from `bonsai-trie`: https://github.com/madara-alliance/bonsai-trie/blob/bfc6ad47b3cb8b75b1326bf630ca16e581f194c5/src/trie/merkle_node.rs#L234-L248
    pub fn compute_hash<Hash: StarkHash>(&self) -> Felt {
        match self {
            Self::Binary { left, right } => Hash::hash(left, right),
            Self::Edge { child, path, length } => {
                let mut length_bytes = [0u8; 32];
                length_bytes[31] = *length;
                let length = Felt::from_bytes_be(&length_bytes);
                Hash::hash(child, path) + length
            }
        }
    }
}

/// The response type for `starknet_getStorageProof` method.
///
/// The requested storage proofs. Note that if a requested leaf has the default value, the path to
/// it may end in an edge node whose path is not a prefix of the requested leaf, thus effectively
/// proving non-membership
#[derive(Debug, Serialize, Deserialize)]
pub struct GetStorageProofResponse {
    pub global_roots: GlobalRoots,
    pub classes_proof: ClassesProof,
    pub contracts_proof: ContractsProof,
    pub contracts_storage_proofs: ContractStorageProofs,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClassesProof {
    pub nodes: Nodes,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ContractsProof {
    /// The nodes in the union of the paths from the contracts tree root to the requested leaves.
    pub nodes: Nodes,
    /// The nonce and class hash for each requested contract address, in the order in which they
    /// appear in the request. These values are needed to construct the associated leaf node.
    pub contract_leaves_data: Vec<ContractLeafData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractLeafData {
    // NOTE: This field is not specified in the RPC specs, but the contract storage root is
    // required to compute the contract state hash (ie the value of the contracts trie). We
    // include this in the response for now to ease the conversions over on SNOS side.
    pub storage_root: Felt,
    pub nonce: Felt,
    pub class_hash: Felt,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractStorageProofs {
    pub nodes: Vec<Nodes>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeWithHash {
    pub node_hash: Felt,
    pub node: MerkleNode,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Nodes(pub Vec<NodeWithHash>);

impl Deref for Nodes {
    type Target = Vec<NodeWithHash>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Nodes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// --- Conversion from/to internal types for convenience

impl From<MultiProof> for Nodes {
    fn from(value: MultiProof) -> Self {
        Self(
            value
                .0
                .into_iter()
                .map(|(hash, node)| NodeWithHash { node_hash: hash, node: MerkleNode::from(node) })
                .collect(),
        )
    }
}

impl From<Nodes> for MultiProof {
    fn from(value: Nodes) -> Self {
        Self(value.0.into_iter().map(|node| (node.node_hash, ProofNode::from(node.node))).collect())
    }
}

impl From<ProofNode> for MerkleNode {
    fn from(value: ProofNode) -> Self {
        match value {
            ProofNode::Binary { left, right } => MerkleNode::Binary { left, right },
            ProofNode::Edge { child, path } => {
                MerkleNode::Edge { child, length: path.len() as u8, path: path_to_felt(path) }
            }
        }
    }
}

impl From<MerkleNode> for ProofNode {
    fn from(value: MerkleNode) -> Self {
        match value {
            MerkleNode::Binary { left, right } => Self::Binary { left, right },
            MerkleNode::Edge { path, child, length } => {
                Self::Edge { child, path: felt_to_path(path, length) }
            }
        }
    }
}

fn felt_to_path(felt: Felt, length: u8) -> Path {
    let length = length as usize;
    let mut bits = BitVec::new();

    // This function converts a Felt to a Path by preserving leading zeros
    // that are semantically important in the Merkle tree path representation.
    //
    // Example:
    // For a path "0000100" (length=7):
    // - As an integer/hex: 0x4 (leading zeros get truncated)
    // - As a Path: [0,0,0,0,1,0,0] (leading zeros preserved)
    //
    // We need to preserve these leading zeros because in a Merkle tree path:
    // - Each bit represents a direction (left=0, right=1)
    // - The position/index of each bit matters for the path traversal
    // - "0000100" and "100" would represent different paths in the tree
    for bit in &felt.to_bits_be()[256 - length..] {
        bits.push(*bit);
    }

    Path(bits)
}

fn path_to_felt(path: Path) -> Felt {
    let mut bytes = [0u8; 32];
    bytes.view_bits_mut()[256 - path.len()..].copy_from_bitslice(&path);
    Felt::from_bytes_be(&bytes)
}

#[cfg(test)]
mod tests {
    use katana_trie::BitVec;

    use super::*;

    // Test cases taken from `bonsai-trie` crate
    #[rstest::rstest]
    #[case(&[0b10101010, 0b10101010])]
    #[case(&[])]
    #[case(&[0b10101010])]
    #[case(&[0b00000000])]
    #[case(&[0b11111111])]
    #[case(&[0b11111111, 0b00000000, 0b10101010, 0b10101010, 0b11111111, 0b00000000, 0b10101010, 0b10101010, 0b11111111, 0b00000000, 0b10101010, 0b10101010])]
    fn path_felt_rt(#[case] input: &[u8]) {
        let path = Path(BitVec::from_slice(input));

        let converted_felt = path_to_felt(path.clone());
        let converted_path = felt_to_path(converted_felt, path.len() as u8);

        assert_eq!(path, converted_path);
        assert_eq!(path.len(), converted_path.len());
    }
}
