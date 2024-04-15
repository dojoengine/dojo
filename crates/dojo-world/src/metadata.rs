use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;

use anyhow::Result;
use camino::Utf8PathBuf;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use scarb::core::{ManifestMetadata, Workspace};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use url::Url;

use crate::manifest::{BaseManifest, WORLD_CONTRACT_NAME};

#[cfg(test)]
#[path = "metadata_test.rs"]
mod test;

pub const IPFS_CLIENT_URL: &str = "https://ipfs.infura.io:5001";
pub const IPFS_USERNAME: &str = "2EBrzr7ZASQZKH32sl2xWauXPSA";
pub const IPFS_PASSWORD: &str = "12290b883db9138a8ae3363b6739d220";

// copy constants from dojo-lang to avoid circular dependency
pub const MANIFESTS_DIR: &str = "manifests";
pub const ABIS_DIR: &str = "abis";
pub const SOURCES_DIR: &str = "src";
pub const BASE_DIR: &str = "base";

fn build_artifact_from_name(
    source_dir: &Utf8PathBuf,
    abi_dir: &Utf8PathBuf,
    element_name: &str,
) -> ArtifactMetadata {
    let sanitized_name = element_name.replace("::", "_");
    let abi_file = abi_dir.join(format!("{sanitized_name}.json"));
    let src_file = source_dir.join(format!("{sanitized_name}.cairo"));

    ArtifactMetadata {
        abi: if abi_file.exists() { Some(Uri::File(abi_file.into_std_path_buf())) } else { None },
        source: if src_file.exists() {
            Some(Uri::File(src_file.into_std_path_buf()))
        } else {
            None
        },
    }
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
pub fn project_to_world_metadata(project_metadata: Option<ProjectWorldMetadata>) -> WorldMetadata {
    if let Some(m) = project_metadata {
        WorldMetadata {
            name: m.name,
            description: m.description,
            cover_uri: m.cover_uri,
            icon_uri: m.icon_uri,
            website: m.website,
            socials: m.socials,
            ..Default::default()
        }
    } else {
        WorldMetadata {
            name: None,
            description: None,
            cover_uri: None,
            icon_uri: None,
            website: None,
            socials: None,
            ..Default::default()
        }
    }
}

/// Collect metadata from the project configuration and from the workspace.
///
/// # Arguments
/// `ws`: the workspace.
///
/// # Returns
/// A [`DojoMetadata`] object containing all Dojo metadata.
pub fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> DojoMetadata {
    let profile = ws.config().profile();

    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let manifest_dir = manifest_dir.join(MANIFESTS_DIR).join(profile.as_str());
    let target_dir = ws.target_dir().path_existent().unwrap();
    let sources_dir = target_dir.join(profile.as_str()).join(SOURCES_DIR);
    let abis_dir = manifest_dir.join(ABIS_DIR).join(BASE_DIR);

    let project_metadata = ws.current_package().unwrap().manifest.metadata.dojo();
    let mut dojo_metadata =
        DojoMetadata { env: project_metadata.env.clone(), ..Default::default() };

    let world_artifact = build_artifact_from_name(&sources_dir, &abis_dir, WORLD_CONTRACT_NAME);

    // inialize Dojo world metadata with world metadata coming from project configuration
    dojo_metadata.world = project_to_world_metadata(project_metadata.world);
    dojo_metadata.world.artifacts = world_artifact;

    // load models and contracts metadata
    if manifest_dir.join(BASE_DIR).exists() {
        if let Ok(manifest) = BaseManifest::load_from_path(&manifest_dir.join(BASE_DIR)) {
            for model in manifest.models {
                let name = model.name.to_string();
                dojo_metadata.artifacts.insert(
                    name.clone(),
                    build_artifact_from_name(&sources_dir, &abis_dir.join("models"), &name),
                );
            }

            for contract in manifest.contracts {
                let name = contract.name.to_string();
                dojo_metadata.artifacts.insert(
                    name.clone(),
                    build_artifact_from_name(&sources_dir, &abis_dir.join("contracts"), &name),
                );
            }
        }
    }

    dojo_metadata
}

/// Metadata coming from project configuration (Scarb.toml)
#[derive(Default, Deserialize, Debug, Clone)]
pub struct ProjectMetadata {
    pub world: Option<ProjectWorldMetadata>,
    pub env: Option<Environment>,
}

/// Metadata collected from the project configuration and the Dojo workspace
#[derive(Default, Deserialize, Debug, Clone)]
pub struct DojoMetadata {
    pub world: WorldMetadata,
    pub env: Option<Environment>,
    pub artifacts: HashMap<String, ArtifactMetadata>,
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

impl DojoMetadata {
    pub fn env(&self) -> Option<&Environment> {
        self.env.as_ref()
    }
}

trait MetadataExt {
    fn dojo(&self) -> ProjectMetadata;
}

impl MetadataExt for ManifestMetadata {
    fn dojo(&self) -> ProjectMetadata {
        self.tool_metadata
            .as_ref()
            .and_then(|e| e.get("dojo"))
            .cloned()
            .map(|v| v.try_into::<ProjectMetadata>().unwrap_or_default())
            .unwrap_or_default()
    }
}
