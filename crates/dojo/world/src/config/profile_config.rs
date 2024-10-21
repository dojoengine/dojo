use std::fs;

use anyhow::Result;
use camino::Utf8PathBuf;
use scarb::compiler::Profile;
use serde::Deserialize;
use toml;

use super::environment::Environment;
use super::migration_config::MigrationConfig;
use super::namespace_config::NamespaceConfig;
use super::world_config::WorldConfig;

/// Profile configuration that is used to configure the world and the environment.
///
/// This [`ProfileConfig`] is expected to be loaded from a TOML file that is located
/// next to the `Scarb.toml` file, named with the profile name.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProfileConfig {
    pub world: WorldConfig,
    pub namespace: NamespaceConfig,
    pub env: Option<Environment>,
    pub migration: Option<MigrationConfig>,
}

impl ProfileConfig {
    /// Loads the profile configuration for the given profile.
    ///
    /// # Arguments
    ///
    /// * `manifest_dir` - The path to the directory containing the `Scarb.toml` file.
    /// * `profile` - The profile to load the configuration for.
    pub fn new(manifest_dir: &Utf8PathBuf, profile: Profile) -> Result<Self> {
        let dev_config_path = manifest_dir.join("dojo_dev.toml");
        let config_path = manifest_dir.join(format!("dojo_{}.toml", profile.as_str()));

        if !dev_config_path.exists() {
            return Err(anyhow::anyhow!(
                "Profile configuration file not found for profile `{}`. Expected at {}.",
                profile.as_str(),
                dev_config_path
            ));
        }

        // If the profile file is not found, default to `dev.toml` file that must exist.
        let config_path = if !config_path.exists() { dev_config_path } else { config_path };

        let content = fs::read_to_string(&config_path)?;
        let config: ProfileConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use smol_str::SmolStr;
    use tempfile::TempDir;
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
        mappings = { "test" = "test2" }

        [env]
        rpc_url = "https://example.com/rpc"
        account_address = "test"
        private_key = "test"
        keystore_path = "test"
        keystore_password = "test"
        world_address = "test"

        [migration]
        skip_contracts = [ "module::my-contract" ]

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
            Some(HashMap::from([("test".to_string(), "test2".to_string())]))
        );
    }

    #[test]
    fn test_profile_config_new_dev() {
        let tmp_dir =
            Utf8PathBuf::from(TempDir::new().unwrap().into_path().to_string_lossy().to_string());
        let config_path = tmp_dir.join("dojo_dev.toml");
        println!("config_path: {:?}", config_path);

        let config_content = r#"
        [world]
        name = "test_world"
        seed = "1234"

        [namespace]
        default = "test_namespace"
        "#;
        fs::write(&config_path, config_content).unwrap();

        let config = ProfileConfig::new(&tmp_dir, Profile::DEV).unwrap();

        assert_eq!(config.world.name, "test_world");
        assert_eq!(config.world.seed, "1234");
        assert_eq!(config.namespace.default, "test_namespace");
    }

    #[test]
    fn test_profile_config_new_custom_profile() {
        let tmp_dir =
            Utf8PathBuf::from(TempDir::new().unwrap().into_path().to_string_lossy().to_string());

        let dev_config_path = tmp_dir.join("dojo_dev.toml");
        let config_path = tmp_dir.join("dojo_slot.toml");
        println!("config_path: {:?}", config_path);

        let config_content = r#"
        [world]
        name = "test_world"
        seed = "1234"

        [namespace]
        default = "test_namespace"
        "#;
        fs::write(&config_path, config_content).unwrap();
        fs::write(&dev_config_path, config_content).unwrap();

        let profile = Profile::new(SmolStr::from("slot")).unwrap();

        let config = ProfileConfig::new(&tmp_dir, profile).unwrap();

        assert_eq!(config.world.name, "test_world");
        assert_eq!(config.world.seed, "1234");
        assert_eq!(config.namespace.default, "test_namespace");
    }
}
