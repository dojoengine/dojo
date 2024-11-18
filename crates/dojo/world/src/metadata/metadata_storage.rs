use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::{Context, Result};
use serde_json::json;
use starknet_crypto::Felt;

use super::metadata_service::MetadataService;
use crate::config::metadata_config::{ResourceMetadata, WorldMetadata};
use crate::uri::Uri;

/// Helper function to compute metadata hash.
///
/// # Arguments
///   * `data` - the data to hash.
///
/// # Returns
///   The hash value.
fn compute_metadata_hash<T>(data: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// Helper function to process an optional URI.
///
/// If the URI is set and refer to a local asset, this asset
/// is then uploaded using the provided MetadataService.
/// In any other case, the URI is kept as it is.
///
/// # Arguments
///   * `uri` - The URI to process
///   * `service` - The metadata service to use to upload assets.
///
/// # Returns
///   The updated URI or a Anyhow error.
async fn upload_uri(uri: &Option<Uri>, service: &mut impl MetadataService) -> Result<Option<Uri>> {
    if let Some(Uri::File(path)) = uri {
        let data = std::fs::read(path)?;
        let uploaded_uri = Uri::Ipfs(service.upload(data).await?);
        Ok(Some(uploaded_uri))
    } else {
        Ok(uri.clone())
    }
}

/// Trait to be implemented by metadata structs to be
/// uploadable on a storage system.
#[allow(async_fn_in_trait)]
pub trait MetadataStorage {
    /// Upload metadata using the provided service.
    ///
    /// # Arguments
    ///   * `service` - service to use to upload metadata
    ///
    /// # Returns
    ///   The uploaded metadata URI or a Anyhow error.
    async fn upload(&self, service: &mut impl MetadataService) -> Result<String>;

    /// Upload metadata using the provided service, only if it has changed.
    ///
    /// # Arguments
    ///   * `service` - service to use to upload metadata
    ///   * `current_hash` - the hash of the previously uploaded metadata
    ///
    /// # Returns
    ///   The uploaded metadata URI or a Anyhow error.
    async fn upload_if_changed(
        &self,
        service: &mut impl MetadataService,
        current_hash: Felt,
    ) -> Result<Option<(String, Felt)>>
    where
        Self: std::hash::Hash,
    {
        let new_hash = compute_metadata_hash(self);
        let new_hash = Felt::from_raw([0, 0, 0, new_hash]);

        if new_hash != current_hash {
            let new_uri = self.upload(service).await?;
            return Ok(Some((new_uri, new_hash)));
        }

        Ok(None)
    }
}

#[allow(async_fn_in_trait)]
impl MetadataStorage for WorldMetadata {
    async fn upload(&self, service: &mut impl MetadataService) -> Result<String> {
        let mut meta = self.clone();

        meta.icon_uri =
            upload_uri(&self.icon_uri, service).await.context("Failed to upload icon URI")?;
        meta.cover_uri =
            upload_uri(&self.cover_uri, service).await.context("Failed to upload cover URI")?;

        let serialized = json!(meta).to_string();
        service.upload(serialized.as_bytes().to_vec()).await.context("Failed to upload metadata")
    }
}

#[allow(async_fn_in_trait)]
impl MetadataStorage for ResourceMetadata {
    async fn upload(&self, service: &mut impl MetadataService) -> Result<String> {
        let mut meta = self.clone();

        meta.icon_uri =
            upload_uri(&self.icon_uri, service).await.context("Failed to upload icon URI")?;

        let serialized = json!(meta).to_string();
        service.upload(serialized.as_bytes().to_vec()).await.context("Failed to upload metadata")
    }
}
