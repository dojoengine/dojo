//! Namespace configuration to map local resource to their world representation.
//!
//! A namespace configuration is a mapping between a local artifacts names and
//! the actual namespace they will have onchain.
//!
//! Event if locally the compiled resources have no namespace, they need one to
//! be registered in the world.
//! Since the world doesn't offers a default namespace, each project should define
//! one.
//!
//! If required, the namespace configuration can be more granular by mapping
//! specific local names to different namespaces. The same resource might appear
//! under different namespaces depending on the deployment scenario. This is valid.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Namespace configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceConfig {
    /// The default namespace to use if none is specified.
    pub default: String,
    /// A mapping `<Namespace, <LocalNames>>`
    pub mappings: Option<HashMap<String, Vec<String>>>,
}

impl NamespaceConfig {
    /// Creates a new namespace configuration with a default namespace.
    pub fn new(default: &str) -> Self {
        NamespaceConfig { default: default.to_string(), mappings: None }
    }

    /// Adds mappings to the namespace configuration.
    pub fn with_mappings(self, mappings: HashMap<String, Vec<String>>) -> Self {
        Self { mappings: Some(mappings), ..self }
    }

    /// Returns all the namespaces mapped to the given local name.
    ///
    /// If a resource is explicitly mapped to a namespace, it will not be
    /// mapped it to the default namespace.
    ///
    /// However, if no explicit mapping is provided, the default namespace is used.
    pub fn get_namespaces(&self, local_name: &str) -> HashSet<String> {
        let mut namespaces = HashSet::new();

        if let Some(mappings) = &self.mappings {
            for (namespace, names) in mappings {
                if names.contains(&local_name.to_string()) {
                    namespaces.insert(namespace.clone());
                }
            }
        }

        if namespaces.is_empty() {
            namespaces.insert(self.default.clone());
        }

        namespaces
    }

    /// Returns all the namespaces registered in the configuration.
    pub fn list_namespaces(&self) -> Vec<String> {
        let mut namespaces = vec![self.default.clone()];

        if let Some(mappings) = &self.mappings {
            namespaces.extend(mappings.keys().cloned());
        }

        namespaces
    }
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        NamespaceConfig::new("DEFAULT_NAMESPACE")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_namespaces_default() {
        let config = NamespaceConfig::new("dojo").with_mappings(HashMap::new());

        assert_eq!(config.get_namespaces("c1"), HashSet::from(["dojo".to_string()]));
    }

    #[test]
    fn test_get_namespaces_explicit_single() {
        let config = NamespaceConfig::new("dojo")
            .with_mappings(HashMap::from([("ns1".to_string(), vec!["c1".to_string()])]));

        assert_eq!(config.get_namespaces("c1"), HashSet::from(["ns1".to_string()]));
    }

    #[test]
    fn test_get_namespaces_explicit_multiple() {
        let config = NamespaceConfig::new("dojo").with_mappings(HashMap::from([(
            "ns1".to_string(),
            vec!["c1".to_string(), "c2".to_string()],
        )]));

        assert_eq!(config.get_namespaces("c1"), HashSet::from(["ns1".to_string()]));
        assert_eq!(config.get_namespaces("c2"), HashSet::from(["ns1".to_string()]));
    }

    #[test]
    fn test_list_namespaces() {
        let config = NamespaceConfig::new("dojo").with_mappings(HashMap::from([(
            "ns1".to_string(),
            vec!["c1".to_string(), "c2".to_string()],
        )]));

        assert_eq!(config.list_namespaces(), vec!["dojo".to_string(), "ns1".to_string()]);
    }
}
