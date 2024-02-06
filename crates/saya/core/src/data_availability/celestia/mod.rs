//! Celestia client to publish state update data.
use std::fmt::Display;

use async_trait::async_trait;
use celestia_rpc::{BlobClient, Client};
use celestia_types::blob::SubmitOptions;
use celestia_types::nmt::Namespace;
use celestia_types::Blob;
use starknet::core::types::FieldElement;
use url::Url;

use crate::data_availability::error::{DataAvailabilityResult, Error};
use crate::data_availability::{DataAvailabilityClient, DataAvailabilityMode};

#[derive(Debug, Clone)]
pub struct CelestiaConfig {
    pub node_url: Url,
    pub node_auth_token: Option<String>,
    pub namespace: String,
}

impl Display for CelestiaConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node_url = &self.node_url;
        let auth_token = self.node_auth_token.clone().unwrap_or("not set".to_string());
        let namespace = &self.namespace;
        write!(f, "* node url: {node_url}\n* namespace: {namespace}\n* auth token: {auth_token}")
    }
}

pub struct CelestiaClient {
    client: Client,
    mode: DataAvailabilityMode,
    namespace: Namespace,
}

impl CelestiaClient {
    pub async fn new(config: CelestiaConfig) -> DataAvailabilityResult<Self> {
        Ok(Self {
            client: Client::new(config.node_url.as_ref(), config.node_auth_token.as_deref())
                .await?,
            mode: DataAvailabilityMode::Validium,
            namespace: Namespace::new_v0(config.namespace.as_bytes())?,
        })
    }
}

#[async_trait]
impl DataAvailabilityClient for CelestiaClient {
    fn mode(&self) -> DataAvailabilityMode {
        self.mode
    }

    async fn publish_state_diff_felts(
        &self,
        state_diff: &[FieldElement],
    ) -> DataAvailabilityResult<u64> {
        let bytes: Vec<u8> = state_diff.iter().flat_map(|fe| fe.to_bytes_be().to_vec()).collect();

        let blob = Blob::new(self.namespace, bytes)?;

        // TODO: we may want to use `blob_get` to ensure the state diff has been published
        // correctly.
        self.client
            .blob_submit(&[blob], SubmitOptions::default())
            .await
            .map_err(|e| Error::Client(format!("Celestia RPC error: {e}")))
    }
}

impl From<celestia_rpc::Error> for Error {
    fn from(e: celestia_rpc::Error) -> Self {
        Self::Client(format!("Celestia RPC error: {e}"))
    }
}

impl From<celestia_types::Error> for Error {
    fn from(e: celestia_types::Error) -> Self {
        Self::Client(format!("Celestia types error: {e}"))
    }
}
