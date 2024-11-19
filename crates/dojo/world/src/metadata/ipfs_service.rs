use std::io::Cursor;

use anyhow::Result;
#[cfg(test)]
use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, TryFromUri};

use super::metadata_service::MetadataService;

/// IPFS implementation of MetadataService, allowing to
/// upload metadata to IPFS.
pub struct IpfsMetadataService {
    client: ipfs_api_backend_hyper::IpfsClient,
}

// impl required by clippy
impl std::fmt::Debug for IpfsMetadataService {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        Ok(())
    }
}

impl IpfsMetadataService {
    /// Instanciate a new IPFS Metadata service with IPFS credentials.
    ///
    /// # Arguments
    ///   * `client_url` - The IPFS client URL
    ///   * `username` - The IPFS username
    ///   * `password` - The IPFS password
    ///
    /// # Returns
    ///  A new `IpfsMetadataService` is the IPFS client has been successfully
    ///  instanciated or a Anyhow error if not.
    pub fn new(client_url: &str, username: &str, password: &str) -> Result<Self> {
        if client_url.is_empty() || username.is_empty() || password.is_empty() {
            anyhow::bail!("Invalid IPFS credentials: empty values not allowed");
        }
        if !client_url.starts_with("http://") && !client_url.starts_with("https://") {
            anyhow::bail!("Invalid IPFS URL: must start with http:// or https://");
        }

        Ok(Self {
            client: ipfs_api_backend_hyper::IpfsClient::from_str(client_url)?
                .with_credentials(username, password),
        })
    }
}

#[allow(async_fn_in_trait)]
impl MetadataService for IpfsMetadataService {
    async fn upload(&mut self, data: Vec<u8>) -> Result<String> {
        let reader = Cursor::new(data);
        let response = self
            .client
            .add(reader)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to upload to IPFS: {}", e))?;
        Ok(format!("ipfs://{}", response.hash))
    }

    #[cfg(test)]
    async fn get(&self, uri: String) -> Result<Vec<u8>> {
        let res = self
            .client
            .cat(&uri.replace("ipfs://", ""))
            .map_ok(|chunk| chunk.to_vec())
            .try_concat()
            .await?;
        Ok(res)
    }
}
