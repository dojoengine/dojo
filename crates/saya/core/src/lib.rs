//! Saya core library.
use std::sync::Arc;

use starknet::core::types::{BlockId, MaybePendingStateUpdate, StateUpdate};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tracing::{error, info, trace, warn};
use url::Url;

use crate::data_availability::{
    DataAvailabilityClient, DataAvailabilityConfig, DataAvailabilityMode,
};
use crate::error::SayaResult;

pub mod data_availability;
pub mod error;
pub mod prover;
pub mod verifier;

/// Saya's main configuration.
pub struct SayaConfig {
    pub katana_rpc: Url,
    pub start_block: u64,
    pub data_availability: DataAvailabilityConfig,
}

/// Saya.
pub struct Saya {
    /// The main Saya configuration.
    config: SayaConfig,
    /// The data availability client.
    da_client: Box<dyn DataAvailabilityClient>,
    /// The katana (for now JSON RPC) client.
    katana_client: Arc<JsonRpcClient<HttpTransport>>,
}

impl Saya {
    /// Initializes a new [`Saya`] instance from the given [`SayaConfig`].
    ///
    /// # Arguments
    ///
    /// * `config` - The main Saya configuration.
    pub async fn new(config: SayaConfig) -> SayaResult<Self> {
        let katana_client =
            Arc::new(JsonRpcClient::new(HttpTransport::new(config.katana_rpc.clone())));

        let da_client: Box<dyn DataAvailabilityClient> =
            data_availability::client_from_config(config.data_availability.clone()).await?;

        Ok(Self { config, da_client, katana_client })
    }

    /// Starts the Saya mainloop to fetch and process data.
    ///
    /// Optims:
    /// First naive version to have an overview of all the components
    /// and the process.
    /// Should be refacto in crates as necessary.
    pub async fn start(&self) -> SayaResult<()> {
        let poll_interval_secs = 1;
        let mut block = self.config.start_block;

        loop {
            let mut latest_block = match self.katana_client.block_number().await {
                Ok(block_number) => block_number,
                Err(e) => {
                    error!("Can't retrieve latest block: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                    continue;
                }
            };

            if block > latest_block {
                trace!("Nothing to process yet, waiting for block {block}");
                tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                continue;
            }

            self.process_block(block).await?;

            block += 1;

            tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
        }

        Ok(())
    }

    /// Processes the given block number.
    ///
    /// # Summary
    ///
    /// 1. Pulls state update to update local state accordingly. We may publish DA at this point.
    ///
    /// 2. Pulls all transactions and data required to generate the trace.
    ///
    /// 3. Computes facts for this state transition. We may optimistically register the facts.
    ///
    /// 4. Computes the proof from the trace with a prover.
    ///
    /// 5. Registers the facts + the send the proof to verifier. Not all provers require this step
    ///    (a.k.a. SHARP).
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number.
    async fn process_block(&self, block_number: u64) -> SayaResult<()> {
        trace!("Processing block {block_number}");

        self.fetch_publish_state_update(block_number).await?;

        // do the other operations, depending on the configuration.

        Ok(())
    }

    /// Fetches the state update for the given block and publish it to the data availability layer.
    /// Returns the [`StateUpdate`].
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number to get state update for.
    async fn fetch_publish_state_update(&self, block_number: u64) -> SayaResult<StateUpdate> {
        let state_update =
            match self.katana_client.get_state_update(BlockId::Number(block_number)).await? {
                MaybePendingStateUpdate::Update(su) => {
                    let sd_felts =
                        data_availability::state_diff::state_diff_to_felts(&su.state_diff);
                    self.da_client.publish_state_diff_felts(&sd_felts).await?;
                    su
                }
                MaybePendingStateUpdate::PendingUpdate(_) => unreachable!("Should not be used"),
            };

        Ok(state_update)
    }
}

impl From<starknet::providers::ProviderError> for error::Error {
    fn from(e: starknet::providers::ProviderError) -> Self {
        Self::KatanaClient(format!("Katana client RPC provider error: {e}"))
    }
}
