use std::collections::HashMap;

use katana_primitives::contract::StorageKey;
use katana_primitives::{ContractAddress, Felt};
use katana_trie::bonsai::BitSlice;
use katana_trie::{MultiProof, Path, ProofNode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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
    pub nonce: Felt,
    pub class_hash: Felt,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ContractStorageProofs {
    pub nodes: Vec<Nodes>,
}

#[derive(Debug, Default)]
pub struct Nodes(pub HashMap<Felt, MerkleNode>);

impl Serialize for Nodes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        #[derive(Debug, Serialize)]
        struct NodeEntry<'a> {
            node_hash: &'a Felt,
            node: &'a MerkleNode,
        }

        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for (node_hash, node) in &self.0 {
            seq.serialize_element(&NodeEntry { node_hash, node })?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Nodes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        struct NodeEntry {
            node_hash: Felt,
            node: MerkleNode,
        }

        let entries: Vec<NodeEntry> = Vec::deserialize(deserializer)?;
        let map = entries.into_iter().map(|entry| (entry.node_hash, entry.node)).collect();
        Ok(Nodes(map))
    }
}

// --- Conversion from/to internal types for convenience

impl From<MultiProof> for Nodes {
    fn from(value: MultiProof) -> Self {
        Self(value.0.into_iter().map(|(hash, node)| (hash, MerkleNode::from(node))).collect())
    }
}

impl From<Nodes> for MultiProof {
    fn from(value: Nodes) -> Self {
        Self(value.0.into_iter().map(|(hash, node)| (hash, ProofNode::from(node))).collect())
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
            MerkleNode::Edge { path, child, .. } => Self::Edge { child, path: felt_to_path(path) },
        }
    }
}

fn felt_to_path(felt: Felt) -> Path {
    Path(BitSlice::from_slice(&felt.to_bytes_be())[5..].to_bitvec())
}

fn path_to_felt(path: Path) -> Felt {
    let mut arr = [0u8; 32];
    let slice = &mut BitSlice::from_slice_mut(&mut arr)[5..];
    slice[..path.len()].copy_from_bitslice(&path);
    Felt::from_bytes_be(&arr)
}

#[cfg(test)]
mod tests {
    use katana_primitives::felt;

    use super::*;

    // This test is assuming that the `path` field in `MerkleNode::Edge` is already a valid trie
    // path value.
    #[rstest::rstest]
    #[case(felt!("0x1234567890abcdef"))]
    #[case(felt!("0xdeadbeef"))]
    #[case(Felt::MAX)]
    #[case(Felt::ZERO)]
    fn test_path_felt_roundtrip(#[case] path_in_felt: Felt) {
        let initial_path = felt_to_path(path_in_felt);

        let converted_felt = path_to_felt(initial_path.clone());
        let path = felt_to_path(converted_felt);
        assert_eq!(initial_path, path);
    }
}
