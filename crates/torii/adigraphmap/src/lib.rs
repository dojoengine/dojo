use error::DigraphMapError;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub mod error;

/// Result type for DigraphMap operations
pub type Result<T> = std::result::Result<T, DigraphMapError>;

/// A directed graph that maps dependencies between nodes with values of type `V`.
/// Each node has a unique key of type `K` that can be used to identify it.
#[derive(Debug)]
pub struct AcyclicDigraphMap<K, V>
where
    K: Eq + Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    /// The underlying directed graph
    graph: DiGraph<V, ()>,

    /// Map from node keys to their indices in the graph
    node_indices: HashMap<K, NodeIndex>,
}

impl<K, V> AcyclicDigraphMap<K, V>
where
    K: Eq + Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    /// Create a new empty DigraphMap
    pub fn new() -> Self {
        Self { graph: DiGraph::new(), node_indices: HashMap::new() }
    }

    /// Add a node with dependencies to the graph
    pub fn add_node_with_dependencies(
        &mut self,
        key: K,
        value: V,
        dependencies: Vec<K>,
    ) -> Result<NodeIndex> {
        let node_idx = self.add_node(key.clone(), value)?;
        for dependency in dependencies {
            self.add_dependency(&dependency, &key)?;
        }
        Ok(node_idx)
    }

    /// Add a node to the graph with the given key and value
    pub fn add_node(&mut self, key: K, value: V) -> Result<NodeIndex> {
        if self.node_indices.contains_key(&key) {
            return Err(DigraphMapError::DuplicateKey(format!("{:?}", key)));
        }

        let node_idx = self.graph.add_node(value);
        self.node_indices.insert(key, node_idx);
        Ok(node_idx)
    }

    /// Add an edge representing a dependency from `from` to `to`.
    /// This means that `from` must be processed before `to`.
    pub fn add_dependency(&mut self, from: &K, to: &K) -> Result<()> {
        let from_idx = self.get_node_index(from)?;
        let to_idx = self.get_node_index(to)?;

        // Check if adding this edge would create a cycle
        if self.would_create_cycle(from_idx, to_idx) {
            return Err(DigraphMapError::CycleDetected);
        }

        self.graph.add_edge(from_idx, to_idx, ());
        Ok(())
    }

    /// Get a reference to the value associated with a key
    pub fn get(&self, key: &K) -> Option<&V> {
        self.node_indices.get(key).map(|&idx| &self.graph[idx])
    }

    /// Get a mutable reference to the value associated with a key
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let idx = self.node_indices.get(key).cloned();
        idx.map(move |idx| &mut self.graph[idx])
    }

    /// Get the node index for a key, or return an error if the key doesn't exist
    fn get_node_index(&self, key: &K) -> Result<NodeIndex> {
        self.node_indices
            .get(key)
            .cloned()
            .ok_or_else(|| DigraphMapError::NodeNotFound(format!("{:?}", key)))
    }

    /// Check if adding an edge from `from` to `to` would create a cycle
    fn would_create_cycle(&self, from: NodeIndex, to: NodeIndex) -> bool {
        // If there's already a path from to -> from, adding from -> to would create a cycle
        petgraph::algo::has_path_connecting(&self.graph, to, from, None)
    }
    /// Get the nodes in topological order (respecting dependencies)
    pub fn topo_sort(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();

        // Perform topological sort (guaranteed to succeed since graph is acyclic)
        let indices = petgraph::algo::toposort(&self.graph, None)
            .expect("Graph is guaranteed to be acyclic due to dependency checks");

        // Collect nodes in topological order
        for idx in indices {
            let node_value = self.graph[idx].clone();
            if let Some(key) =
                self.node_indices.iter().find(|&(_, &v)| v == idx).map(|(k, _)| k.clone())
            {
                result.push((key, node_value));
            }
        }

        result
    }

    /// Get the nodes grouped by topological level, where each level contains nodes
    /// that can be processed in parallel (no dependencies among them).
    /// Returns a vector of vectors, where each inner vector represents a level.
    pub fn topo_sort_by_level(&self) -> Vec<Vec<(K, V)>> {
        let mut levels: Vec<Vec<(K, V)>> = Vec::new();
        let mut remaining_nodes: HashSet<NodeIndex> = self.node_indices.values().cloned().collect();
        let mut incoming_edges: HashMap<NodeIndex, usize> = HashMap::new();

        // Initialize incoming edge counts
        for node in self.graph.node_indices() {
            let incoming = self.graph.neighbors_directed(node, petgraph::Incoming).count();
            incoming_edges.insert(node, incoming);
        }

        // Find nodes with no incoming edges (roots) as level 0
        let mut current_level_nodes: Vec<NodeIndex> = incoming_edges
            .iter()
            .filter(|&(_, &count)| count == 0)
            .map(|(&node, _)| node)
            .collect();

        while !current_level_nodes.is_empty() {
            let mut next_level_nodes: Vec<NodeIndex> = Vec::new();
            let mut level_nodes: Vec<(K, V)> = Vec::new();

            // Process all nodes in the current level
            for node in current_level_nodes {
                if remaining_nodes.remove(&node) {
                    // Find the key and value for this node
                    if let Some(key) =
                        self.node_indices.iter().find(|&(_, &v)| v == node).map(|(k, _)| k.clone())
                    {
                        let value = self.graph[node].clone();
                        level_nodes.push((key, value));
                    }

                    // Update dependencies for neighbors
                    for neighbor in self.graph.neighbors_directed(node, petgraph::Outgoing) {
                        let count = incoming_edges.get_mut(&neighbor).unwrap();
                        *count -= 1;
                        if *count == 0 {
                            next_level_nodes.push(neighbor);
                        }
                    }
                }
            }

            if !level_nodes.is_empty() {
                levels.push(level_nodes);
            }

            current_level_nodes = next_level_nodes;
        }

        levels
    }

    /// Check if the graph is empty
    pub fn is_empty(&self) -> bool {
        self.graph.node_count() == 0
    }

    /// Get the number of nodes in the graph
    pub fn len(&self) -> usize {
        self.graph.node_count()
    }

    /// Remove all nodes and edges from the graph
    pub fn clear(&mut self) {
        self.graph.clear();
        self.node_indices.clear();
    }
}

impl<K, V> Default for AcyclicDigraphMap<K, V>
where
    K: Eq + Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get() {
        let mut graph: AcyclicDigraphMap<String, i32> = AcyclicDigraphMap::new();
        graph.add_node("a".to_string(), 1).unwrap();
        graph.add_node("b".to_string(), 2).unwrap();

        assert_eq!(graph.get(&"a".to_string()), Some(&1));
        assert_eq!(graph.get(&"b".to_string()), Some(&2));
        assert_eq!(graph.get(&"c".to_string()), None);
    }

    #[test]
    fn test_add_dependency() {
        let mut graph: AcyclicDigraphMap<String, i32> = AcyclicDigraphMap::new();
        graph.add_node("a".to_string(), 1).unwrap();
        graph.add_node("b".to_string(), 2).unwrap();

        assert!(graph.add_dependency(&"a".to_string(), &"b".to_string()).is_ok());

        // The order should be a, b
        let result = graph.topo_sort();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "a");
        assert_eq!(result[1].0, "b");
    }

    #[test]
    fn test_add_node_with_dependencies() {
        let mut graph: AcyclicDigraphMap<String, i32> = AcyclicDigraphMap::new();
        graph.add_node_with_dependencies("b".to_string(), 2, vec![]).unwrap();
        graph.add_node_with_dependencies("a".to_string(), 1, vec!["b".to_string()]).unwrap();

        let result = graph.topo_sort();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "b");
        assert_eq!(result[1].0, "a");
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph: AcyclicDigraphMap<String, i32> = AcyclicDigraphMap::new();
        graph.add_node("a".to_string(), 1).unwrap();
        graph.add_node("b".to_string(), 2).unwrap();

        assert!(graph.add_dependency(&"a".to_string(), &"b".to_string()).is_ok());
        assert!(graph.add_dependency(&"b".to_string(), &"a".to_string()).is_err());
    }

    #[test]
    fn test_topo_sort_by_level() {
        let mut graph: AcyclicDigraphMap<String, i32> = AcyclicDigraphMap::new();

        // Create a graph:
        //     a -> b -> d
        //     a -> c -> d
        // Expected levels:
        // Level 0: [a]
        // Level 1: [b, c]
        // Level 2: [d]

        graph.add_node("a".to_string(), 1).unwrap();
        graph.add_node("b".to_string(), 2).unwrap();
        graph.add_node("c".to_string(), 3).unwrap();
        graph.add_node("d".to_string(), 4).unwrap();

        graph.add_dependency(&"a".to_string(), &"b".to_string()).unwrap();
        graph.add_dependency(&"a".to_string(), &"c".to_string()).unwrap();
        graph.add_dependency(&"b".to_string(), &"d".to_string()).unwrap();
        graph.add_dependency(&"c".to_string(), &"d".to_string()).unwrap();

        let levels = graph.topo_sort_by_level();

        assert_eq!(levels.len(), 3, "Should have 3 levels");

        // Level 0: a
        assert_eq!(levels[0].len(), 1);
        assert_eq!(levels[0][0].0, "a");

        // Level 1: b, c (order within level may vary)
        assert_eq!(levels[1].len(), 2);
        let level1_keys: Vec<&str> = levels[1].iter().map(|(k, _)| k.as_str()).collect();
        assert!(level1_keys.contains(&"b"));
        assert!(level1_keys.contains(&"c"));

        // Level 2: d
        assert_eq!(levels[2].len(), 1);
        assert_eq!(levels[2][0].0, "d");
    }
}
