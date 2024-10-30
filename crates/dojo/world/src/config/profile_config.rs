use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;
use toml;

use super::environment::Environment;
use super::migration_config::MigrationConfig;
use super::namespace_config::NamespaceConfig;
use super::world_config::WorldConfig;

/// Profile configuration that is used to configure the world and its environment.
///
/// This [`ProfileConfig`] is expected to be loaded from a TOML file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProfileConfig {
    pub world: WorldConfig,
    pub namespace: NamespaceConfig,
    pub env: Option<Environment>,
    pub migration: Option<MigrationConfig>,
    /// A mapping <name_or_tag, [tags]> of writers to be set on the world.
    pub writers: Option<HashMap<String, HashSet<String>>>,
    /// A mapping <name_or_tag, [tags]> of owners to be set on the world.
    pub owners: Option<HashMap<String, HashSet<String>>>,
    /// A mapping <tag, [values]> of init call arguments to be passed to the contract.
    pub init_call_args: Option<HashMap<String, Vec<String>>>,
}

impl ProfileConfig {
    pub fn new(name: &str, seed: &str, namespace: NamespaceConfig) -> Self {
        Self {
            world: WorldConfig {
                name: name.to_string(),
                seed: seed.to_string(),
                ..Default::default()
            },
            namespace,
            ..Default::default()
        }
    }

    /// Loads the profile configuration from a TOML file.
    pub fn from_toml<P: AsRef<Path>>(toml_path: P) -> Result<Self> {
        let content = fs::read_to_string(&toml_path)?;
        let config: ProfileConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Returns the local writers for a given tag.
    pub fn get_local_writers(&self, tag: &str) -> HashSet<String> {
        if let Some(writers) = &self.writers {
            writers.get(tag).unwrap_or(&HashSet::new()).clone()
        } else {
            HashSet::new()
        }
    }

    /// Returns the local owners for a given tag.
    pub fn get_local_owners(&self, tag: &str) -> HashSet<String> {
        if let Some(owners) = &self.owners {
            owners.get(tag).unwrap_or(&HashSet::new()).clone()
        } else {
            HashSet::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use url::Url;

    use super::*;
    use crate::uri::Uri;

    #[test]
    fn test_profile_config_empty() {
        let content = "";

        let error = toml::from_str::<ProfileConfig>(content).unwrap_err();
        assert!(error.to_string().contains("missing field `world`"));
    }

    #[test]
    fn test_profile_config_seed_missing() {
        let content = r#"
        [world]
        name = "test"
        "#;

        let error = toml::from_str::<ProfileConfig>(content).unwrap_err();
        assert!(error.to_string().contains("missing field `seed`"));
    }

    #[test]
    fn test_profile_config_min() {
        let content = r#"
        [world]
        name = "test"
        seed = "abcd"

        [namespace]
        default = "test"
        "#;

        let _ = toml::from_str::<ProfileConfig>(content).unwrap();
    }

    #[test]
    fn test_profile_config_full() {
        let content = r#"
        [world]
        name = "test"
        seed = "abcd"
        description = "test"
        cover_uri = "file://example.com/cover.png"
        icon_uri = "ipfs://example.com/icon.png"
        website = "https://example.com"
        socials = { "twitter" = "test", "discord" = "test" }

        [namespace]
        default = "test"
        mappings = { "test" = ["test2"] }

        [env]
        rpc_url = "https://example.com/rpc"
        account_address = "test"
        private_key = "test"
        keystore_path = "test"
        keystore_password = "test"
        world_address = "test"

        [migration]
        skip_contracts = [ "module::my-contract" ]

        [writers]
        "ns1" = ["ns1-actions"]

        [owners]
        "ns2" = ["ns2-blup"]

        [init_call_args]
        "ns1-actions" = [ "0x1", "0x2" ]
        "#;

        let config = toml::from_str::<ProfileConfig>(content).unwrap();

        let migration = config.migration.unwrap();
        assert_eq!(migration.skip_contracts.unwrap(), vec!["module::my-contract".to_string()]);

        let env = config.env.unwrap();
        assert_eq!(env.rpc_url, Some("https://example.com/rpc".to_string()));
        assert_eq!(env.account_address, Some("test".to_string()));
        assert_eq!(env.private_key, Some("test".to_string()));
        assert_eq!(env.keystore_path, Some("test".to_string()));
        assert_eq!(env.keystore_password, Some("test".to_string()));
        assert_eq!(env.world_address, Some("test".to_string()));

        assert_eq!(config.world.description, Some("test".to_string()));
        assert_eq!(
            config.world.cover_uri,
            Some(Uri::from_string("file://example.com/cover.png").unwrap())
        );
        assert_eq!(
            config.world.icon_uri,
            Some(Uri::from_string("ipfs://example.com/icon.png").unwrap())
        );
        assert_eq!(config.world.website, Some(Url::try_from("https://example.com").unwrap()));
        assert_eq!(
            config.world.socials,
            Some(HashMap::from([
                ("twitter".to_string(), "test".to_string()),
                ("discord".to_string(), "test".to_string())
            ]))
        );

        assert_eq!(config.namespace.default, "test".to_string());
        assert_eq!(
            config.namespace.mappings,
            Some(HashMap::from([("test".to_string(), vec!["test2".to_string()])]))
        );

        assert_eq!(
            config.writers,
            Some(HashMap::from([("ns1".to_string(), HashSet::from(["ns1-actions".to_string()]))]))
        );
        assert_eq!(
            config.owners,
            Some(HashMap::from([("ns2".to_string(), HashSet::from(["ns2-blup".to_string()]))]))
        );
        assert_eq!(
            config.init_call_args,
            Some(HashMap::from([(
                "ns1-actions".to_string(),
                vec!["0x1".to_string(), "0x2".to_string()]
            )]))
        );
    }
}
