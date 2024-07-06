use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use scarb::core::{ManifestMetadata, Workspace};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use url::Url;

use crate::contracts::naming;
use crate::manifest::{BaseManifest, CONTRACTS_DIR, MODELS_DIR, WORLD_CONTRACT_TAG};

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

/// Get the default namespace from the workspace.
///
/// # Arguments
///
/// * `ws`: the workspace.
///
/// # Returns
///
/// A [`String`] object containing the namespace.
pub fn get_default_namespace_from_ws(ws: &Workspace<'_>) -> String {
    let metadata = dojo_metadata_from_workspace(ws)
        .expect("Namespace key is already checked by the parsing of the Scarb.toml file.");
    metadata.world.namespace
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

/// Collect metadata from the project configuration and from the workspace.
///
/// # Arguments
/// `ws`: the workspace.
///
/// # Returns
/// A [`DojoMetadata`] object containing all Dojo metadata.
pub fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Result<DojoMetadata> {
    let profile = ws.config().profile();

    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let manifest_dir = manifest_dir.join(MANIFESTS_DIR).join(profile.as_str());
    let abi_dir = manifest_dir.join(ABIS_DIR).join(BASE_DIR);
    let source_dir = ws.target_dir().path_existent().unwrap();
    let source_dir = source_dir.join(profile.as_str());

    let project_metadata = if let Ok(current_package) = ws.current_package() {
        current_package.manifest.metadata.dojo()?
    } else {
        // On workspaces, dojo metadata are not accessible because if no current package is defined
        // (being the only package or using --package).
        return Err(anyhow!(
            "No current package with dojo metadata found, this subcommand is not yet support for \
             workspaces."
        ));
    };

    let mut dojo_metadata = DojoMetadata {
        env: project_metadata.env.clone(),
        skip_migration: project_metadata.skip_migration.clone(),
        ..Default::default()
    };

    let world_artifact = build_artifact_from_filename(
        &abi_dir,
        &source_dir,
        &naming::get_filename_from_tag(WORLD_CONTRACT_TAG),
    );

    // inialize Dojo world metadata with world metadata coming from project configuration
    dojo_metadata.world = project_to_world_metadata(project_metadata.world);
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

    Ok(dojo_metadata)
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

/// World metadata coming from the project configuration (Scarb.toml)
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ProjectWorldMetadata {
    pub name: Option<String>,
    pub seed: String,
    pub namespace: String,
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
    pub namespace: String,
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
            // TODO: see if we can make error more descriptive
            .ok_or_else(|| anyhow!("Some of the fields in [tool.dojo] are required."))?
            .clone();

        let project_metadata: ProjectMetadata = metadata
            .try_into()
            .with_context(|| "Project metadata (i.e. [tool.dojo]) is not properly configured.")?;

        Ok(project_metadata)
    }
}
