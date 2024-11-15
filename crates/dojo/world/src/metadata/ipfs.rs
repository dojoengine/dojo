use std::io::Cursor;

use anyhow::Result;
#[cfg(test)]
use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, TryFromUri};

const IPFS_CLIENT_URL: &str = "https://ipfs.infura.io:5001";
const IPFS_USERNAME: &str = "2EBrzr7ZASQZKH32sl2xWauXPSA";
const IPFS_PASSWORD: &str = "12290b883db9138a8ae3363b6739d220";

pub struct IpfsClient {
    client: ipfs_api_backend_hyper::IpfsClient,
}

impl IpfsClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: ipfs_api_backend_hyper::IpfsClient::from_str(IPFS_CLIENT_URL)?
                .with_credentials(IPFS_USERNAME, IPFS_PASSWORD),
        })
    }

    /// Upload a `data` on IPFS and get a IPFS URI.
    ///
    /// # Arguments
    ///  * `data`: the data to upload
    ///
    /// # Returns
    ///  Result<String> - returns the IPFS URI or a Anyhow error.
    pub(crate) async fn upload<T>(&self, data: T) -> Result<String>
    where
        T: AsRef<[u8]> + std::marker::Send + std::marker::Sync + std::marker::Unpin + 'static,
    {
        let reader = Cursor::new(data);
        let response = self.client.add(reader).await?;
        Ok(format!("ipfs://{}", response.hash))
    }

    #[cfg(test)]
    pub(crate) async fn get(&self, uri: String) -> Result<Vec<u8>> {
        let res = self
            .client
            .cat(&uri.replace("ipfs://", ""))
            .map_ok(|chunk| chunk.to_vec())
            .try_concat()
            .await?;
        Ok(res)
    }
}
