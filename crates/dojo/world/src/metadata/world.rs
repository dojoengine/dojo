use std::io::Cursor;

use anyhow::Result;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use serde_json::json;

use crate::config::metadata_config::WorldMetadata;
use crate::uri::Uri;

#[cfg(test)]
#[path = "metadata_test.rs"]
mod test;

pub const IPFS_CLIENT_URL: &str = "https://ipfs.infura.io:5001";
pub const IPFS_USERNAME: &str = "2EBrzr7ZASQZKH32sl2xWauXPSA";
pub const IPFS_PASSWORD: &str = "12290b883db9138a8ae3363b6739d220";

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
