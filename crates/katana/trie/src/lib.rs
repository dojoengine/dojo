use bitvec::{order::Msb0, vec::BitVec};
use slab::Slab;

pub type Path = BitVec<u8, Msb0>;
pub type NodeId = usize;

#[derive(Debug)]
pub enum NodeChildren {
    Single(NodeId),
    Double { left: NodeId, right: NodeId },
}

#[derive(Debug)]
pub enum Node {
    Leaf { value: u64 },
    Internal { height: u64, path: Path, children: NodeChildren },
}

#[derive(Debug, Default)]
pub struct BinaryTrie {
    root: Option<NodeId>,
    nodes: Slab<Node>,
}

impl BinaryTrie {
    pub fn new() -> Self {
        BinaryTrie::default()
    }

    pub fn insert(&mut self, key: Path, value: u64) {
        let nodes = self.traverse(&key);

        match nodes.last() {
            Some(id) => {
                let node = self.nodes.get_mut(*id).unwrap();

                match node {
                    Node::Leaf { value } => {
                        todo!()
                    }

                    Node::Internal { path, children } => {}
                }
            }

            None => {
                let leaf = Node::Leaf { value };
                let leaf_id = self.nodes.insert(leaf);

                let root = Node::Internal { path: key, children: NodeChildren::Single(leaf_id) };

                let root_id = self.nodes.insert(root);
                self.root = Some(root_id);
            }
        }
    }

    pub fn get(&self, key: [u8; 32]) -> Option<&Node> {
        // tarverse and get the node id
        None
    }

    fn traverse(&self, path: &Path) -> Vec<NodeId> {
        let Some(root) = self.root else { return Vec::new() };

        let mut nodes: Vec<NodeId> = Vec::new();
        let mut current: NodeId = root;

        loop {
            // its a bug if unwrap fails
            let node = self.nodes.get(current).unwrap();

            match node {
                Node::Leaf { .. } => {
                    nodes.push(current);
                    break;
                }

                Node::Internal { height, path: node_path, children } => {
                    let a = &path[];

                    // check if the path has the same prefix as the current node path
                    if node_path == a {
                        match children {
                            NodeChildren::Single(node) => {
                                current = *node;
                            }

                            NodeChildren::Double { left, right } => {
                                let next = path[node_path.len()];
                                if next {
                                    current = *right;
                                } else {
                                    current = *left;
                                }
                            }
                        }
                    } else {
                        nodes.push(current);
                        break;
                    }
                }
            }

            nodes.push(current);
        }

        nodes
    }
}

fn is_prefix(prefix: &Path, path: &Path) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(a, b)| a == b)
}

fn common_prefix(a: &Path, b: &Path) -> Path {
    a.iter().zip(b).take_while(|(a, b)| a == b).fold(Path::new(), |mut path, (a, _)| {
        path.push(*a);
        path
    })
}

// // path returns the path as mentioned in the [specification] for commitment calculations.
// // path is suffix of key that diverges from parentKey. For example,
// // for a key 0b1011 and parentKey 0b10, this function would return the path object of 0b0.
// func path(key, parentKey *Key) Key {
// 	path := *key
// 	// drop parent key, and one more MSB since left/right relation already encodes that information
// 	if parentKey != nil {
// 		path.Truncate(path.Len() - parentKey.Len() - 1)
// 	}
// 	return path
// }

// pub fn path_matches(&self, key

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BinaryTrie;

    #[test]
    fn get_insert() {
        let mut trie = BinaryTrie::new();

        let path = Path::from_iter([true, false, true, false, false, false, false, false]);
        let value = 42;
        trie.insert(path, value);

        let path = Path::from_iter([true, false, false, false, false, false, false, false]);
        let value = 69;
        trie.insert(path, value);
    }

    #[test]
    fn test_common_prefix() {
        let a = Path::from_iter([true, false, true, true, false, true, true, false]);
        let b = Path::from_iter([true, false, true, false, true, false, true, true]);
        let expected = Path::from_iter([true, false, true]);
        assert_eq!(common_prefix(&a, &b), expected);

        let c = Path::from_iter([false, true, false, true, true, false, true, false]);
        let d = Path::from_iter([true, false, true, false, false, true, true, true]);
        let expected_empty = Path::new();
        assert_eq!(common_prefix(&c, &d), expected_empty);

        let e = Path::from_iter([true, true, true, false, true, false, true, true]);
        let f = Path::from_iter([true, true, true, false, true, false, true, true]);
        assert_eq!(common_prefix(&e, &f), e);
    }
}
