use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::Result;
use async_trait::async_trait;
use ipfs::IpfsClient;
use serde_json::json;
use starknet_crypto::Felt;

use crate::config::metadata_config::{ResourceMetadata, WorldMetadata};
use crate::uri::Uri;

mod ipfs;

#[cfg(test)]
mod metadata_test;

/// Helper function to compute metadata hash using the Hash trait impl.
fn compute_metadata_hash<T>(data: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

#[async_trait]
pub trait MetadataStorage {
    async fn upload(&self) -> Result<String>;

    async fn upload_if_changed(&self, current_hash: Felt) -> Result<Option<(String, Felt)>>
    where
        Self: std::hash::Hash,
    {
        let new_hash = compute_metadata_hash(self);
        let new_hash = Felt::from_raw([0, 0, 0, new_hash]);

        if new_hash != current_hash {
            let new_uri = self.upload().await?;
            return Ok(Some((new_uri, new_hash)));
        }

        Ok(None)
    }
}

#[async_trait]
impl MetadataStorage for WorldMetadata {
    async fn upload(&self) -> Result<String> {
        let mut meta = self.clone();

        let ipfs_client = IpfsClient::new()?;

        if let Some(Uri::File(icon)) = &self.icon_uri {
            let icon_data = std::fs::read(icon)?;
            meta.icon_uri = Some(Uri::Ipfs(ipfs_client.upload(icon_data).await?));
        };

        if let Some(Uri::File(cover)) = &self.cover_uri {
            let cover_data = std::fs::read(cover)?;
            meta.cover_uri = Some(Uri::Ipfs(ipfs_client.upload(cover_data).await?));
        };

        let serialized = json!(meta).to_string();
        ipfs_client.upload(serialized).await
    }
}

#[async_trait]
impl MetadataStorage for ResourceMetadata {
    async fn upload(&self) -> Result<String> {
        let mut meta = self.clone();

        let ipfs_client = IpfsClient::new()?;

        if let Some(Uri::File(icon)) = &self.icon_uri {
            let icon_data = std::fs::read(icon)?;
            meta.icon_uri = Some(Uri::Ipfs(ipfs_client.upload(icon_data).await?));
        };

        let serialized = json!(meta).to_string();
        ipfs_client.upload(serialized).await
    }
}
