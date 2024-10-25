use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Result;
use dojo_types::naming;
use serde::Deserialize;
use toml;

use super::environment::Environment;
use super::migration_config::MigrationConfig;
use super::namespace_config::NamespaceConfig;
use super::world_config::WorldConfig;
use crate::DojoSelector;

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
    /// Loads the profile configuration from a TOML file.
    pub fn from_toml<P: AsRef<Path>>(toml_path: P) -> Result<Self> {
        let content = fs::read_to_string(&toml_path)?;
        let config: ProfileConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Extracts the local writers from the profile configuration, computing the selectors.
    pub fn get_local_writers(&self) -> HashMap<DojoSelector, LocalPermission> {
        if let Some(user_names_tags) = &self.writers {
            from_names_tags_to_selectors(user_names_tags)
        } else {
            HashMap::new()
        }
    }

    /// Extracts the local owners from the profile configuration, computing the selectors.
    pub fn get_local_owners(&self) -> HashMap<DojoSelector, LocalPermission> {
        if let Some(user_names_tags) = &self.owners {
            from_names_tags_to_selectors(user_names_tags)
        } else {
            HashMap::new()
        }
    }
}

/// A local permission, containing the tag of the resource to grant permissions to and the grantees.
#[derive(Debug, Clone, Default)]
pub struct LocalPermission {
    pub target_tag: String,
    pub grantees: HashSet<(DojoSelector, String)>,
}

/// Converts a mapping of names or tags to tags into a mapping of selectors to selectors.
///
/// Returns the selectors of the resource to grant permissions to and it's tag.
fn from_names_tags_to_selectors(
    names_tags: &HashMap<String, HashSet<String>>,
) -> HashMap<DojoSelector, LocalPermission> {
    let mut perms = HashMap::new();

    for (name_or_tag, tags) in names_tags.iter() {
        let mut local_permission = LocalPermission {
            target_tag: name_or_tag.clone(),
            grantees: HashSet::new(),
        };

        let target_selector = if naming::is_valid_tag(name_or_tag) {
            naming::compute_selector_from_tag(name_or_tag)
        } else {
            naming::compute_bytearray_hash(name_or_tag)
        };

        for tag in tags {
            let granted_selector = naming::compute_selector_from_tag(tag);
            local_permission.grantees.insert((granted_selector, tag.clone()));
        }

        perms.insert(target_selector, local_permission);
    }

    perms
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
        assert_eq!(migration.skip_contracts, vec!["module::my-contract".to_string()]);

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

    #[test]
    fn test_from_names_tags_to_selectors() {
        let mut names_tags = HashMap::new();

        let mut tags1 = HashSet::new();
        tags1.insert("ns1-spawner".to_string());
        tags1.insert("ns1-mover".to_string());
        names_tags.insert("ns1".to_string(), tags1);

        let mut tags2 = HashSet::new();
        tags2.insert("ns2-spawner".to_string());
        names_tags.insert("ns2".to_string(), tags2);

        let result = from_names_tags_to_selectors(&names_tags);

        let ns1_selector = naming::compute_bytearray_hash("ns1");
        let ns2_selector = naming::compute_bytearray_hash("ns2");
        let ns1_spawner_selector = naming::compute_selector_from_tag("ns1-spawner");
        let ns1_mover_selector = naming::compute_selector_from_tag("ns1-mover");
        let ns2_spawner_selector = naming::compute_selector_from_tag("ns2-spawner");

        assert_eq!(result.get(&ns1_selector).unwrap().contains(&ns1_spawner_selector), true);
        assert_eq!(result.get(&ns1_selector).unwrap().contains(&ns1_mover_selector), true);
        assert_eq!(result.get(&ns2_selector).unwrap().contains(&ns2_spawner_selector), true);
    }
}
