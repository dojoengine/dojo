use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;

use anyhow::{Context, Result};
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use regex::Regex;
use scarb::core::{ManifestMetadata, Package, TargetKind, Workspace};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use cairo_lang_filesystem::cfg::CfgSet;
use serde_json::json;
use url::Url;

const LOG_TARGET: &str = "dojo_world::metadata";

#[cfg(test)]
#[path = "metadata_test.rs"]
mod test;

pub const IPFS_CLIENT_URL: &str = "https://ipfs.infura.io:5001";
pub const IPFS_USERNAME: &str = "2EBrzr7ZASQZKH32sl2xWauXPSA";
pub const IPFS_PASSWORD: &str = "12290b883db9138a8ae3363b6739d220";

// copy constants from dojo-lang to avoid circular dependency
pub const MANIFESTS_DIR: &str = "manifests";
pub const ABIS_DIR: &str = "abis";
pub const BASE_DIR: &str = "base";
pub const NAMESPACE_CFG_PREFIX: &str = "nm|";
pub const DEFAULT_NAMESPACE_CFG_KEY: &str = "namespace_default";

/// Get the default namespace from the workspace.
///
/// # Arguments
///
/// * `ws`: the workspace.
///
/// # Returns
///
/// A [`String`] object containing the namespace.
pub fn get_default_namespace_from_ws(ws: &Workspace<'_>) -> Result<String> {
    let metadata = dojo_metadata_from_workspace(ws)?;
    Ok(metadata.world.namespace.default)
}

/// Get the namespace configuration from the workspace.
///
/// # Arguments
///
/// * `ws`: the workspace.
///
/// # Returns
///
/// A [`NamespaceConfig`] object containing the namespace configuration.
pub fn get_namespace_config_from_ws(ws: &Workspace<'_>) -> Result<NamespaceConfig> {
    let metadata = dojo_metadata_from_workspace(ws)?;
    Ok(metadata.world.namespace)
}

/// Build world metadata with data read from the project configuration.
///
/// # Arguments
///
/// * `project_metadata` - The project metadata.
///
/// # Returns
///
/// A [`WorldMetadata`] object initialized with project metadata.
pub fn project_to_world_metadata(m: ProjectWorldMetadata) -> WorldMetadata {
    WorldMetadata {
        name: m.name,
        description: m.description,
        cover_uri: m.cover_uri,
        icon_uri: m.icon_uri,
        website: m.website,
        socials: m.socials,
        seed: m.seed,
        namespace: m.namespace,
        ..Default::default()
    }
}

pub fn dojo_metadata_from_package(package: &Package, ws: &Workspace<'_>) -> Result<DojoMetadata> {
    tracing::debug!(target: LOG_TARGET, package_id = package.id.to_string(), "Collecting Dojo metadata from package.");

    // If it's a lib, we can try to extract dojo data. If failed -> then we can return default.
    // But like so, if some metadata are here, we get them.
    // [[target.dojo]] shouldn't be used with [lib] as no files will be deployed.
    let is_lib = package.target(&TargetKind::new("lib")).is_some();
    let is_dojo = package.target(&TargetKind::new("dojo")).is_some();

    if is_lib && is_dojo {
        return Err(anyhow::anyhow!("[lib] package cannot have [[target.dojo]]."));
    }

    let project_metadata = match package.manifest.metadata.dojo() {
        Ok(m) => Ok(m),
        Err(e) => {
            if is_lib || !is_dojo {
                Ok(ProjectMetadata::default())
            } else {
                Err(anyhow::anyhow!(
                    "In manifest {} [dojo] package must have [[target.dojo]]: {}.",
                    ws.manifest_path(),
                    e
                ))
            }
        }
    }?;

    let dojo_metadata = DojoMetadata {
        env: project_metadata.env.clone(),
        skip_migration: project_metadata.skip_migration.clone(),
        world: project_to_world_metadata(project_metadata.world),
        ..Default::default()
    };

    tracing::trace!(target: LOG_TARGET, ?dojo_metadata);

    Ok(dojo_metadata)
}

pub fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Result<DojoMetadata> {
    let dojo_packages: Vec<Package> = ws
        .members()
        .filter(|package| {
            package.target(&TargetKind::new("dojo")).is_some()
                && package.target(&TargetKind::new("lib")).is_none()
        })
        .collect();

    match dojo_packages.len() {
        0 => {
            ws.config().ui().warn(
                "No package with dojo target found in workspace. If your package is a [lib] with \
                 [[target.dojo]], you can ignore this warning.",
            );
            Ok(DojoMetadata::default())
        }
        1 => {
            let dojo_package =
                dojo_packages.into_iter().next().expect("Package must exist as len is 1.");
            Ok(dojo_metadata_from_package(&dojo_package, ws)?)
        }
        _ => Err(anyhow::anyhow!(
            "Multiple packages with dojo target found in workspace. Please specify a package \
             using --package option or maybe one of them must be declared as a [lib]."
        )),
    }
}

/// Metadata coming from project configuration (Scarb.toml)
#[derive(Default, Deserialize, Debug, Clone)]
pub struct ProjectMetadata {
    pub world: ProjectWorldMetadata,
    pub env: Option<Environment>,
    pub skip_migration: Option<Vec<String>>,
}

/// Metadata for a user defined resource (models, contracts).
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ResourceMetadata {
    pub name: String,
    pub artifacts: ArtifactMetadata,
}

/// Metadata collected from the project configuration and the Dojo workspace
#[derive(Default, Deserialize, Debug, Clone)]
pub struct DojoMetadata {
    pub world: WorldMetadata,
    pub env: Option<Environment>,
    pub resources_artifacts: HashMap<String, ResourceMetadata>,
    pub skip_migration: Option<Vec<String>>,
}

#[derive(Debug)]
pub enum UriParseError {
    InvalidUri,
    InvalidFileUri,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Uri {
    Http(Url),
    Ipfs(String),
    File(PathBuf),
}

impl Serialize for Uri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Uri::Http(url) => serializer.serialize_str(url.as_ref()),
            Uri::Ipfs(ipfs) => serializer.serialize_str(ipfs),
            Uri::File(path) => serializer.serialize_str(&format!("file://{}", path.display())),
        }
    }
}

impl<'de> Deserialize<'de> for Uri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.starts_with("ipfs://") {
            Ok(Uri::Ipfs(s))
        } else if let Some(path) = s.strip_prefix("file://") {
            Ok(Uri::File(PathBuf::from(&path)))
        } else if let Ok(url) = Url::parse(&s) {
            Ok(Uri::Http(url))
        } else {
            Err(serde::de::Error::custom("Invalid Uri"))
        }
    }
}

impl Uri {
    pub fn cid(&self) -> Option<&str> {
        match self {
            Uri::Ipfs(value) => value.strip_prefix("ipfs://"),
            _ => None,
        }
    }
}

/// Checks if the provided namespace follows the format rules.
pub fn is_name_valid(namespace: &str) -> bool {
    Regex::new(r"^[a-zA-Z0-9_]+$").unwrap().is_match(namespace)
}

/// Namespace configuration for the world
#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NamespaceConfig {
    pub default: String,
    pub mappings: Option<HashMap<String, String>>,
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

        if !is_name_valid(&self.default) {
            return Err(anyhow::anyhow!("Invalid default namespace `{}`", self.default));
        }

        for (tag_or_namespace, namespace) in self.mappings.as_ref().unwrap_or(&HashMap::new()) {
            if !is_name_valid(namespace) {
                return Err(anyhow::anyhow!(
                    "Invalid namespace `{}` for tag or namespace `{}`",
                    namespace,
                    tag_or_namespace
                ));
            }
        }

        Ok(self)
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

/// World metadata coming from the project configuration (Scarb.toml)
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ProjectWorldMetadata {
    pub name: Option<String>,
    pub seed: String,
    pub namespace: NamespaceConfig,
    pub description: Option<String>,
    pub cover_uri: Option<Uri>,
    pub icon_uri: Option<Uri>,
    pub website: Option<Url>,
    pub socials: Option<HashMap<String, String>>,
}

/// World metadata collected from the project configuration and the Dojo workspace
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct WorldMetadata {
    pub name: Option<String>,
    pub seed: String,
    pub namespace: NamespaceConfig,
    pub description: Option<String>,
    pub cover_uri: Option<Uri>,
    pub icon_uri: Option<Uri>,
    pub website: Option<Url>,
    pub socials: Option<HashMap<String, String>>,
    pub artifacts: ArtifactMetadata,
}

/// Metadata Artifacts collected for one Dojo element (world, model, contract...)
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ArtifactMetadata {
    pub abi: Option<Uri>,
    pub source: Option<Uri>,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct Environment {
    pub rpc_url: Option<String>,
    pub account_address: Option<String>,
    pub private_key: Option<String>,
    pub keystore_path: Option<String>,
    pub keystore_password: Option<String>,
    pub world_address: Option<String>,
}

impl Environment {
    pub fn world_address(&self) -> Option<&str> {
        self.world_address.as_deref()
    }

    pub fn rpc_url(&self) -> Option<&str> {
        self.rpc_url.as_deref()
    }

    pub fn account_address(&self) -> Option<&str> {
        self.account_address.as_deref()
    }

    pub fn private_key(&self) -> Option<&str> {
        self.private_key.as_deref()
    }

    pub fn keystore_path(&self) -> Option<&str> {
        self.keystore_path.as_deref()
    }

    pub fn keystore_password(&self) -> Option<&str> {
        self.keystore_password.as_deref()
    }
}

impl ProjectWorldMetadata {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

impl WorldMetadata {
    pub async fn upload(&self) -> Result<String> {
        let mut meta = self.clone();
        let client =
            IpfsClient::from_str(IPFS_CLIENT_URL)?.with_credentials(IPFS_USERNAME, IPFS_PASSWORD);

        if let Some(Uri::File(icon)) = &self.icon_uri {
            let icon_data = std::fs::read(icon)?;
            let reader = Cursor::new(icon_data);
            let response = client.add(reader).await?;
            meta.icon_uri = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        if let Some(Uri::File(cover)) = &self.cover_uri {
            let cover_data = std::fs::read(cover)?;
            let reader = Cursor::new(cover_data);
            let response = client.add(reader).await?;
            meta.cover_uri = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        if let Some(Uri::File(abi)) = &self.artifacts.abi {
            let abi_data = std::fs::read(abi)?;
            let reader = Cursor::new(abi_data);
            let response = client.add(reader).await?;
            meta.artifacts.abi = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        if let Some(Uri::File(source)) = &self.artifacts.source {
            let source_data = std::fs::read(source)?;
            let reader = Cursor::new(source_data);
            let response = client.add(reader).await?;
            meta.artifacts.source = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        let serialized = json!(meta).to_string();
        let reader = Cursor::new(serialized);
        let response = client.add(reader).await?;

        Ok(response.hash)
    }
}

impl ArtifactMetadata {
    pub async fn upload(&self) -> Result<String> {
        let mut meta = self.clone();
        let client =
            IpfsClient::from_str(IPFS_CLIENT_URL)?.with_credentials(IPFS_USERNAME, IPFS_PASSWORD);

        if let Some(Uri::File(abi)) = &self.abi {
            let abi_data = std::fs::read(abi)?;
            let reader = Cursor::new(abi_data);
            let response = client.add(reader).await?;
            meta.abi = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        if let Some(Uri::File(source)) = &self.source {
            let source_data = std::fs::read(source)?;
            let reader = Cursor::new(source_data);
            let response = client.add(reader).await?;
            meta.source = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        let serialized = json!(meta).to_string();
        let reader = Cursor::new(serialized);
        let response = client.add(reader).await?;

        Ok(response.hash)
    }
}

impl ResourceMetadata {
    pub async fn upload(&self) -> Result<String> {
        let mut meta = self.clone();
        let client =
            IpfsClient::from_str(IPFS_CLIENT_URL)?.with_credentials(IPFS_USERNAME, IPFS_PASSWORD);

        if let Some(Uri::File(abi)) = &self.artifacts.abi {
            let abi_data = std::fs::read(abi)?;
            let reader = Cursor::new(abi_data);
            let response = client.add(reader).await?;
            meta.artifacts.abi = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        if let Some(Uri::File(source)) = &self.artifacts.source {
            let source_data = std::fs::read(source)?;
            let reader = Cursor::new(source_data);
            let response = client.add(reader).await?;
            meta.artifacts.source = Some(Uri::Ipfs(format!("ipfs://{}", response.hash)))
        };

        let serialized = json!(meta).to_string();
        let reader = Cursor::new(serialized);
        let response = client.add(reader).await?;

        Ok(response.hash)
    }
}

impl DojoMetadata {
    pub fn env(&self) -> Option<&Environment> {
        self.env.as_ref()
    }
}

trait MetadataExt {
    fn dojo(&self) -> Result<ProjectMetadata>;
}

impl MetadataExt for ManifestMetadata {
    fn dojo(&self) -> Result<ProjectMetadata> {
        let metadata = self
            .tool_metadata
            .as_ref()
            .and_then(|e| e.get("dojo"))
            .with_context(|| "No [tool.dojo] section found in the manifest.".to_string())?
            .clone();

        // The details of which field has failed to be loaded are logged inside the `try_into`
        // error.
        let project_metadata: ProjectMetadata = metadata
            .try_into()
            .with_context(|| "Project metadata [tool.dojo] is not properly configured.")?;

        Ok(project_metadata)
    }
}

#[cfg(test)]
mod tests {
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
        assert!(is_name_valid("validName"));
        assert!(is_name_valid("valid_name"));
        assert!(is_name_valid("ValidName123"));
        assert!(is_name_valid("VALID_NAME"));
        assert!(is_name_valid("v"));
    }

    #[test]
    fn test_is_name_valid_with_invalid_names() {
        assert!(!is_name_valid("invalid-name"));
        assert!(!is_name_valid("invalid name"));
        assert!(!is_name_valid("invalid.name"));
        assert!(!is_name_valid("invalid!name"));
        assert!(!is_name_valid(""));
    }
}
