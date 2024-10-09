use std::collections::HashMap;

use anyhow::Result;
use cairo_lang_filesystem::cfg::CfgSet;
use regex::Regex;
use serde::Deserialize;

pub const NAMESPACE_CFG_PREFIX: &str = "nm|";
pub const DEFAULT_NAMESPACE_CFG_KEY: &str = "namespace_default";
pub const DOJO_MANIFESTS_DIR_CFG_KEY: &str = "dojo_manifests_dir";
pub const DEFAULT_NAMESPACE: &str = "DEFAULT_NAMESPACE";

/// Namespace configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceConfig {
    pub default: String,
    pub mappings: Option<HashMap<String, String>>,
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        NamespaceConfig { default: DEFAULT_NAMESPACE.to_string(), mappings: None }
    }
}

impl NamespaceConfig {
    /// Creates a new namespace configuration with a default namespace.
    pub fn new(default: &str) -> Self {
        NamespaceConfig { default: default.to_string(), mappings: None }
    }

    /// Adds mappings to the namespace configuration.
    pub fn with_mappings(mut self, mappings: HashMap<String, String>) -> Self {
        self.mappings = Some(mappings);
        self
    }

    /// Displays the namespace mappings as a string.
    pub fn display_mappings(&self) -> String {
        if let Some(mappings) = &self.mappings {
            let mut result = String::from("\n-- Mappings --\n");
            for (k, v) in mappings.iter() {
                result += &format!("{} -> {}\n", k, v);
            }
            result
        } else {
            "No mapping to apply".to_string()
        }
    }

    /// Gets the namespace for a given tag or namespace, or return the default
    /// namespace if no mapping was found.
    ///
    /// If the input is a tag, a first perfect match is checked. If no match
    /// for the tag, then a check is done against the namespace of the tag.
    /// If the input is a namespace, a perfect match if checked.
    ///
    /// Examples:
    /// - `get_mapping("armory-Flatbow")` first checks for `armory-Flatbow` tag, then for `armory`
    ///   namespace in mapping keys.
    /// - `get_mapping("armory")` checks for `armory` namespace in mapping keys.
    ///
    /// # Arguments
    ///
    /// * `tag_or_namespace`: the tag or namespace to get the namespace for.
    ///
    /// # Returns
    ///
    /// A [`String`] object containing the namespace.
    pub fn get_mapping(&self, tag_or_namespace: &str) -> String {
        if let Some(namespace_from_tag) =
            self.mappings.as_ref().and_then(|m| m.get(tag_or_namespace))
        {
            namespace_from_tag.clone()
        } else if tag_or_namespace.contains('-') {
            // TODO: we can't access the dojo-world/contracts from here as it belongs to a different
            // feature. The naming module has to be relocated in more generic place,
            // always available.
            let (namespace, _) = tag_or_namespace.split_at(tag_or_namespace.rfind('-').unwrap());
            self.mappings
                .as_ref()
                .and_then(|m| m.get(namespace))
                .unwrap_or(&self.default)
                .to_string()
        } else {
            self.default.clone()
        }
    }

    /// Validates the namespace configuration and their names.
    ///
    /// # Returns
    ///
    /// A [`Result`] object containing the namespace configuration if valid, error otherwise.
    pub fn validate(self) -> Result<Self> {
        if self.default.is_empty() {
            return Err(anyhow::anyhow!("Default namespace is empty"));
        }

        if !Self::is_name_valid(&self.default) {
            return Err(anyhow::anyhow!("Invalid default namespace `{}`", self.default));
        }

        for (tag_or_namespace, namespace) in self.mappings.as_ref().unwrap_or(&HashMap::new()) {
            if !Self::is_name_valid(namespace) {
                return Err(anyhow::anyhow!(
                    "Invalid namespace `{}` for tag or namespace `{}`",
                    namespace,
                    tag_or_namespace
                ));
            }
        }

        Ok(self)
    }

    /// Checks if the provided namespace follows the format rules.
    pub fn is_name_valid(namespace: &str) -> bool {
        Regex::new(r"^[a-zA-Z0-9_]+$").unwrap().is_match(namespace)
    }
}

impl From<&CfgSet> for NamespaceConfig {
    fn from(cfg_set: &CfgSet) -> Self {
        let mut default = "".to_string();
        let mut mappings = HashMap::new();

        for cfg in cfg_set.into_iter() {
            if cfg.key == DEFAULT_NAMESPACE_CFG_KEY {
                if let Some(v) = &cfg.value {
                    default = v.to_string();
                }
            } else if cfg.key.starts_with(NAMESPACE_CFG_PREFIX) {
                let key = cfg.key.replace(NAMESPACE_CFG_PREFIX, "");
                if let Some(v) = &cfg.value {
                    mappings.insert(key, v.to_string());
                }
            }
        }

        let mappings = if mappings.is_empty() { None } else { Some(mappings) };

        NamespaceConfig { default: default.to_string(), mappings }
    }
}

#[cfg(test)]
mod tests {
    use cairo_lang_filesystem::cfg::Cfg;
    use smol_str::SmolStr;

    use super::*;

    #[test]
    fn test_namespace_config_get_mapping() {
        let config = NamespaceConfig {
            default: "nm".to_string(),
            mappings: Some(HashMap::from([
                ("tag1".to_string(), "namespace1".to_string()),
                ("namespace2".to_string(), "namespace2".to_string()),
                ("armory-Flatbow".to_string(), "weapons".to_string()),
            ])),
        };

        assert_eq!(config.get_mapping("tag1"), "namespace1");
        assert_eq!(config.get_mapping("tag1-TestModel"), "namespace1");
        assert_eq!(config.get_mapping("namespace2"), "namespace2");
        assert_eq!(config.get_mapping("armory-Flatbow"), "weapons");
        assert_eq!(config.get_mapping("armory"), "nm");
        assert_eq!(config.get_mapping("unknown"), "nm");
    }

    #[test]
    fn test_namespace_config_validate() {
        let valid_config = NamespaceConfig {
            default: "valid_default".to_string(),
            mappings: Some(HashMap::from([
                ("tag1".to_string(), "valid_namespace1".to_string()),
                ("tag2".to_string(), "valid_namespace2".to_string()),
            ])),
        };
        assert!(valid_config.validate().is_ok());

        let empty_default_config = NamespaceConfig { default: "".to_string(), mappings: None };
        assert!(empty_default_config.validate().is_err());

        let invalid_default_config =
            NamespaceConfig { default: "invalid-default".to_string(), mappings: None };
        assert!(invalid_default_config.validate().is_err());

        let invalid_mapping_config = NamespaceConfig {
            default: "valid_default".to_string(),
            mappings: Some(HashMap::from([
                ("tag1".to_string(), "valid_namespace".to_string()),
                ("tag2".to_string(), "invalid-namespace".to_string()),
            ])),
        };
        assert!(invalid_mapping_config.validate().is_err());
    }

    #[test]
    fn test_namespace_config_new() {
        let config = NamespaceConfig::new("default_namespace");
        assert_eq!(config.default, "default_namespace");
        assert_eq!(config.mappings, None);
    }

    #[test]
    fn test_namespace_config_with_mappings() {
        let mut mappings = HashMap::new();
        mappings.insert("tag1".to_string(), "namespace1".to_string());
        mappings.insert("tag2".to_string(), "namespace2".to_string());

        let config = NamespaceConfig::new("default_namespace").with_mappings(mappings.clone());
        assert_eq!(config.default, "default_namespace");
        assert_eq!(config.mappings, Some(mappings));
    }

    #[test]
    fn test_is_name_valid_with_valid_names() {
        assert!(NamespaceConfig::is_name_valid("validName"));
        assert!(NamespaceConfig::is_name_valid("valid_name"));
        assert!(NamespaceConfig::is_name_valid("ValidName123"));
        assert!(NamespaceConfig::is_name_valid("VALID_NAME"));
        assert!(NamespaceConfig::is_name_valid("v"));
    }

    #[test]
    fn test_is_name_valid_with_invalid_names() {
        assert!(!NamespaceConfig::is_name_valid("invalid-name"));
        assert!(!NamespaceConfig::is_name_valid("invalid name"));
        assert!(!NamespaceConfig::is_name_valid("invalid.name"));
        assert!(!NamespaceConfig::is_name_valid("invalid!name"));
        assert!(!NamespaceConfig::is_name_valid(""));
    }

    #[test]
    fn test_namespace_config_from_cfg_set() {
        let mut cfg_set = CfgSet::new();
        cfg_set.insert(Cfg::kv(DEFAULT_NAMESPACE_CFG_KEY, SmolStr::from("default_namespace")));
        cfg_set
            .insert(Cfg::kv(format!("{}tag1", NAMESPACE_CFG_PREFIX), SmolStr::from("namespace1")));
        cfg_set
            .insert(Cfg::kv(format!("{}tag2", NAMESPACE_CFG_PREFIX), SmolStr::from("namespace2")));

        let namespace_config = NamespaceConfig::from(&cfg_set);

        assert_eq!(namespace_config.default, "default_namespace");
        assert_eq!(
            namespace_config.mappings,
            Some(HashMap::from([
                ("tag1".to_string(), "namespace1".to_string()),
                ("tag2".to_string(), "namespace2".to_string()),
            ]))
        );

        // Test with empty CfgSet
        let empty_cfg_set = CfgSet::new();
        let empty_namespace_config = NamespaceConfig::from(&empty_cfg_set);

        assert_eq!(empty_namespace_config.default, "");
        assert_eq!(empty_namespace_config.mappings, None);
    }
}
