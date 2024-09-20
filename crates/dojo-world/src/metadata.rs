use std::collections::HashMap;
use std::io::Cursor;

use anyhow::Result;
use camino::Utf8PathBuf;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use scarb::core::{Package, TargetKind, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use crate::config::{Environment, MigrationConfig, NamespaceConfig, ProfileConfig, WorldConfig};
use crate::contracts::naming;
use crate::manifest::{BaseManifest, CONTRACTS_DIR, MODELS_DIR, WORLD_CONTRACT_TAG};
use crate::uri::Uri;

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
    Ok(metadata.namespace.default)
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
    Ok(metadata.namespace)
}

/// Loads the Dojo metadata for the given package, where the `profile.toml` file is expected to be
/// located in the package directory, next to the `Scarb.toml` file.
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

    // If not dojo dependent, we should skip metadata gathering.
    if !package
        .manifest
        .summary
        .dependencies
        .iter()
        .any(|dep| dep.name.as_str() == "dojo")
    {
        // Some tests (like dojo-core) may depend on dojo, but there is no dojo dependency in the manifest.
        // In case the profile config file exists, we extract the default namespace from it.
        if let Ok(profile_config) = ProfileConfig::new(
            &Utf8PathBuf::from(package.manifest_path().parent().unwrap()),
            ws.current_profile()?,
        ) {
            let mut metadata = DojoMetadata::default();
            metadata.namespace = profile_config.namespace;
            return Ok(metadata);
        } else {
            tracing::trace!(target: LOG_TARGET, package = ?package.manifest_path(), "No dojo dependency or profile config file found, skipping metadata collection.");
            return Ok(DojoMetadata::default());
        }
    }

    let profile_config = ProfileConfig::new(
        &Utf8PathBuf::from(package.manifest_path().parent().unwrap()),
        ws.current_profile()?,
    )?;

    let mut dojo_metadata = DojoMetadata {
        world: WorldMetadata::from(profile_config.world),
        namespace: profile_config.namespace.clone(),
        env: profile_config.env.clone(),
        migration: profile_config.migration.clone(),
        resources_artifacts: HashMap::new(),
    };

    metadata_artifacts_load(&mut dojo_metadata, ws)?;

    tracing::trace!(target: LOG_TARGET, ?dojo_metadata);

    Ok(dojo_metadata)
}

/// Loads the Dojo metadata from the workspace, where one [[target.dojo]] package is required.
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
            ws.config().ui().warn("No package with [[target.dojo]] found in workspace.");
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

/// Loads the artifacts metadata for the world.
fn metadata_artifacts_load(dojo_metadata: &mut DojoMetadata, ws: &Workspace<'_>) -> Result<()> {
    let profile = ws.config().profile();

    // Use package.manifest_path() if supported by the compiler.
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let manifest_dir = manifest_dir.join(MANIFESTS_DIR).join(profile.as_str());
    let abi_dir = manifest_dir.join(BASE_DIR).join(ABIS_DIR);
    let source_dir = ws.target_dir().path_existent().unwrap();
    let source_dir = source_dir.join(profile.as_str());

    let world_artifact = build_artifact_from_filename(
        &abi_dir,
        &source_dir,
        &naming::get_filename_from_tag(WORLD_CONTRACT_TAG),
    );

    dojo_metadata.world.artifacts = world_artifact;

    // load models and contracts metadata
    if manifest_dir.join(BASE_DIR).exists() {
        if let Ok(manifest) = BaseManifest::load_from_path(&manifest_dir.join(BASE_DIR)) {
            for model in manifest.models {
                let tag = model.inner.tag.clone();
                let abi_model_dir = abi_dir.join(MODELS_DIR);
                let source_model_dir = source_dir.join(MODELS_DIR);
                dojo_metadata.resources_artifacts.insert(
                    tag.clone(),
                    ResourceMetadata {
                        name: tag.clone(),
                        artifacts: build_artifact_from_filename(
                            &abi_model_dir,
                            &source_model_dir,
                            &naming::get_filename_from_tag(&tag),
                        ),
                    },
                );
            }

            for contract in manifest.contracts {
                let tag = contract.inner.tag.clone();
                let abi_contract_dir = abi_dir.join(CONTRACTS_DIR);
                let source_contract_dir = source_dir.join(CONTRACTS_DIR);
                dojo_metadata.resources_artifacts.insert(
                    tag.clone(),
                    ResourceMetadata {
                        name: tag.clone(),
                        artifacts: build_artifact_from_filename(
                            &abi_contract_dir,
                            &source_contract_dir,
                            &naming::get_filename_from_tag(&tag),
                        ),
                    },
                );
            }
        }
    }

    Ok(())
}

fn build_artifact_from_filename(
    abi_dir: &Utf8PathBuf,
    source_dir: &Utf8PathBuf,
    filename: &str,
) -> ArtifactMetadata {
    let abi_file = abi_dir.join(format!("{filename}.json"));
    let src_file = source_dir.join(format!("{filename}.cairo"));

    ArtifactMetadata {
        abi: if abi_file.exists() { Some(Uri::File(abi_file.into_std_path_buf())) } else { None },
        source: if src_file.exists() {
            Some(Uri::File(src_file.into_std_path_buf()))
        } else {
            None
        },
    }
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
    pub resources_artifacts: HashMap<String, ResourceMetadata>,
    pub namespace: NamespaceConfig,
    pub env: Option<Environment>,
    pub migration: Option<MigrationConfig>,
}

/// Metadata Artifacts collected for one Dojo element (world, model, contract...)
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ArtifactMetadata {
    pub abi: Option<Uri>,
    pub source: Option<Uri>,
}

/// World metadata collected from the project configuration and the Dojo workspace
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct WorldMetadata {
    pub name: String,
    pub seed: String,
    pub description: Option<String>,
    pub cover_uri: Option<Uri>,
    pub icon_uri: Option<Uri>,
    pub website: Option<Url>,
    pub socials: Option<HashMap<String, String>>,
    pub artifacts: ArtifactMetadata,
}

impl From<WorldConfig> for WorldMetadata {
    fn from(config: WorldConfig) -> Self {
        WorldMetadata {
            name: config.name,
            seed: config.seed,
            description: config.description,
            cover_uri: config.cover_uri,
            icon_uri: config.icon_uri,
            website: config.website,
            socials: config.socials,
            ..Default::default()
        }
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
