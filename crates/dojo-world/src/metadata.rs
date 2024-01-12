use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;

use anyhow::Result;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use scarb::core::{ManifestMetadata, Workspace};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use url::Url;

#[cfg(test)]
#[path = "metadata_test.rs"]
mod test;

pub fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Option<Metadata> {
    Some(ws.current_package().ok()?.manifest.metadata.dojo())
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct Metadata {
    pub world: Option<WorldMetadata>,
    pub env: Option<Environment>,
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

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct WorldMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub cover_uri: Option<Uri>,
    pub icon_uri: Option<Uri>,
    pub website: Option<Url>,
    pub socials: Option<HashMap<String, String>>,
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

impl WorldMetadata {
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
        let client = IpfsClient::from_str("https://ipfs.infura.io:5001")?
            .with_credentials("2EBrzr7ZASQZKH32sl2xWauXPSA", "12290b883db9138a8ae3363b6739d220");

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

        let serialized = json!(meta).to_string();
        let reader = Cursor::new(serialized);
        let response = client.add(reader).await?;

        Ok(response.hash)
    }
}

impl Metadata {
    pub fn env(&self) -> Option<&Environment> {
        self.env.as_ref()
    }

    pub fn world(&self) -> Option<&WorldMetadata> {
        self.world.as_ref()
    }
}
trait MetadataExt {
    fn dojo(&self) -> Metadata;
}

impl MetadataExt for ManifestMetadata {
    fn dojo(&self) -> Metadata {
        self.tool_metadata
            .as_ref()
            .and_then(|e| e.get("dojo"))
            .cloned()
            .map(|v| v.try_into::<Metadata>().unwrap_or_default())
            .unwrap_or_default()
    }
}
