use error::DigraphMapError;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use std::hash::Hash;
use tracing::warn;

pub mod error;

/// Result type for DigraphMap operations
pub type Result<T> = std::result::Result<T, DigraphMapError>;

/// A directed graph that maps dependencies between nodes with values of type `V`.
/// Each node has a unique key of type `K` that can be used to identify it.
#[derive(Debug)]
pub struct DigraphMap<K, V> 
where
    K: Eq + Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    /// The underlying directed graph
    graph: DiGraph<V, ()>,
    
    /// Map from node keys to their indices in the graph
    node_indices: HashMap<K, NodeIndex>,
}

impl<K, V> DigraphMap<K, V>
where
    K: Eq + Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    /// Create a new empty DigraphMap
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    /// Add a node with dependencies to the graph
    pub fn add_node_with_dependencies(&mut self, key: K, value: V, dependencies: Vec<K>) -> Result<NodeIndex> {
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
        
        // Try to do a topological sort
        match petgraph::algo::toposort(&self.graph, None) {
            Ok(indices) => {
                // Collect the nodes in topological order
                for idx in indices {
                    let node_value = self.graph[idx].clone();
                    
                    // Find the key for this index
                    // This is inefficient but should work for reasonable sized graphs
                    if let Some(key) = self.node_indices.iter()
                        .find(|&(_, &v)| v == idx)
                        .map(|(k, _)| k.clone()) 
                    {
                        result.push((key, node_value));
                    }
                }
            }
            Err(_) => {
                // If there's a cycle, this is a fallback method
                warn!("Cycle detected in dependency graph, falling back to arbitrary order");
                for (key, &idx) in &self.node_indices {
                    result.push((key.clone(), self.graph[idx].clone()));
                }
            }
        }
        
        result
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

impl<K, V> Default for DigraphMap<K, V>
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
        let mut graph: DigraphMap<String, i32> = DigraphMap::new();
        graph.add_node("a".to_string(), 1).unwrap();
        graph.add_node("b".to_string(), 2).unwrap();
        
        assert_eq!(graph.get(&"a".to_string()), Some(&1));
        assert_eq!(graph.get(&"b".to_string()), Some(&2));
        assert_eq!(graph.get(&"c".to_string()), None);
    }
    
    #[test]
    fn test_add_dependency() {
        let mut graph: DigraphMap<String, i32> = DigraphMap::new();
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
        let mut graph: DigraphMap<String, i32> = DigraphMap::new();
        graph.add_node_with_dependencies("b".to_string(), 2, vec![]).unwrap();
        graph.add_node_with_dependencies("a".to_string(), 1, vec!["b".to_string()]).unwrap();
        
        let result = graph.topo_sort();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "b");
        assert_eq!(result[1].0, "a");
    }
    
    #[test]
    fn test_cycle_detection() {
        let mut graph: DigraphMap<String, i32> = DigraphMap::new();
        graph.add_node("a".to_string(), 1).unwrap();
        graph.add_node("b".to_string(), 2).unwrap();
        
        assert!(graph.add_dependency(&"a".to_string(), &"b".to_string()).is_ok());
        assert!(graph.add_dependency(&"b".to_string(), &"a".to_string()).is_err());
    }
} 