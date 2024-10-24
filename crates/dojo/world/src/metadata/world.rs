use std::collections::HashMap;
use std::io::Cursor;

use anyhow::Result;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use crate::config::WorldConfig;
use crate::uri::Uri;

#[cfg(test)]
#[path = "metadata_test.rs"]
mod test;

pub const IPFS_CLIENT_URL: &str = "https://ipfs.infura.io:5001";
pub const IPFS_USERNAME: &str = "2EBrzr7ZASQZKH32sl2xWauXPSA";
pub const IPFS_PASSWORD: &str = "12290b883db9138a8ae3363b6739d220";

/// World metadata that describes the world.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct WorldMetadata {
    pub name: String,
    pub seed: String,
    pub description: Option<String>,
    pub cover_uri: Option<Uri>,
    pub icon_uri: Option<Uri>,
    pub website: Option<Url>,
    pub socials: Option<HashMap<String, String>>,
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

        let serialized = json!(meta).to_string();
        let reader = Cursor::new(serialized);
        let response = client.add(reader).await?;

        Ok(response.hash)
    }
}
