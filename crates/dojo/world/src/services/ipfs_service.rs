use std::io::Cursor;

use anyhow::Result;
#[cfg(test)]
use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, TryFromUri};

use super::upload_service::UploadService;
use crate::config::IpfsConfig;

/// IPFS implementation of UploadService, allowing to upload data to IPFS.
pub struct IpfsService {
    client: ipfs_api_backend_hyper::IpfsClient,
}

// impl required by clippy
impl std::fmt::Debug for IpfsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "IPFS service")?;
        Ok(())
    }
}

impl IpfsService {
    /// Instanciate a new IPFS service with IPFS configuration.
    ///
    /// # Arguments
    ///   * `config` - The IPFS configuration
    ///
    /// # Returns
    ///  A new `IpfsService` is the IPFS client has been successfully
    ///  instanciated or a Anyhow error if not.
    pub fn new(config: IpfsConfig) -> Result<Self> {
        config.assert_valid()?;

        Ok(Self {
            client: ipfs_api_backend_hyper::IpfsClient::from_str(&config.url)?
                .with_credentials(config.username, &config.password),
        })
    }
}

#[allow(async_fn_in_trait)]
impl UploadService for IpfsService {
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
