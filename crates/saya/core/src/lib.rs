//! Saya core library.

use std::sync::Arc;

use futures::future::join_all;
use katana_primitives::block::{BlockNumber, FinalityStatus, SealedBlockWithStatus};
use saya_provider::rpc::JsonRpcProvider;
use saya_provider::Provider as SayaProvider;
use serde::{Deserialize, Serialize};
use tracing::{error, trace};
use url::Url;

use crate::blockchain::Blockchain;
use crate::data_availability::{DataAvailabilityClient, DataAvailabilityConfig};
use crate::error::SayaResult;
use crate::prover::state_diff::ProvedStateDiff;

pub mod blockchain;
pub mod data_availability;
pub mod error;
pub mod prover;
pub mod starknet_os;
pub mod verifier;

/// Saya's main configuration.
#[derive(Debug, Deserialize, Serialize)]
pub struct SayaConfig {
    #[serde(deserialize_with = "url_deserializer")]
    pub katana_rpc: Url,
    pub start_block: u64,
    pub data_availability: Option<DataAvailabilityConfig>,
}

fn url_deserializer<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Url::parse(&s).map_err(serde::de::Error::custom)
}

/// Saya.
pub struct Saya {
    /// The main Saya configuration.
    config: SayaConfig,
    /// The data availability client.
    da_client: Option<Box<dyn DataAvailabilityClient>>,
    /// The provider to fetch dojo from Katana.
    provider: Arc<dyn SayaProvider>,
    /// The blockchain state.
    blockchain: Blockchain,
}

impl Saya {
    /// Initializes a new [`Saya`] instance from the given [`SayaConfig`].
    ///
    /// # Arguments
    ///
    /// * `config` - The main Saya configuration.
    pub async fn new(config: SayaConfig) -> SayaResult<Self> {
        // Currently it's only RPC. But it can be the database
        // file directly in the future or other transports.
        let provider = Arc::new(JsonRpcProvider::new(config.katana_rpc.clone()).await?);

        let da_client = if let Some(da_conf) = &config.data_availability {
            Some(data_availability::client_from_config(da_conf.clone()).await?)
        } else {
            None
        };

        let blockchain = Blockchain::new();

        Ok(Self { config, da_client, provider, blockchain })
    }

    /// Starts the Saya mainloop to fetch and process data.
    ///
    /// Optims:
    /// First naive version to have an overview of all the components
    /// and the process.
    /// Should be refacto in crates as necessary.
    pub async fn start(&mut self) -> SayaResult<()> {
        let poll_interval_secs = 1;
        let mut block = self.config.start_block;

        loop {
            let latest_block = match self.provider.block_number().await {
                Ok(block_number) => block_number,
                Err(e) => {
                    error!(?e, "fetch block number");
                    tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                    continue;
                }
            };

            if block > latest_block {
                trace!(block_number = block, "waiting block number");
                tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                continue;
            }

            self.process_block(block).await?;

            block += 1;
        }
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
    async fn process_block(&mut self, block_number: BlockNumber) -> SayaResult<()> {
        trace!(block_number, "processing block");

        let prev_block_index = if block_number == 0 { 0 } else { block_number - 1 };
        let relevant_blocks = join_all(
            vec![block_number, prev_block_index, 0]
                .into_iter()
                .map(|block_number| self.provider.fetch_block(block_number)),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        let (block, prev_block, genesis_block) =
            (relevant_blocks[0].clone(), relevant_blocks[1].clone(), relevant_blocks[2].clone());

        let (state_updates, da_state_update) =
            self.provider.fetch_state_updates(block_number).await?;

        if let Some(da) = &self.da_client {
            da.publish_state_diff_felts(&da_state_update).await?;
        }

        let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 };

        let state_updates_to_prove = state_updates.state_updates.clone();
        self.blockchain.update_state_with_block(block.clone(), state_updates)?;

        if block_number == 0 {
            return Ok(());
        }

        let _exec_infos = self.provider.fetch_transactions_executions(block_number).await?;

        let to_prove = ProvedStateDiff {
            genesis_state_hash: genesis_block.header.header.state_root,
            prev_state_hash: prev_block.header.header.state_root,
            state_updates: state_updates_to_prove,
        };

        trace!(block_number, "proving block");
        let proof = prover::prove(to_prove.to_string(), prover::ProverIdentifier::Stone).await?;

        trace!(block_number, "verifying block");
        let transaction_hash =
            verifier::verify(proof, verifier::VerifierIdentifier::HerodotusStarknetSepolia).await?;

        trace!(block_number, transaction_hash, "block verified");

        Ok(())
    }
}

impl From<starknet::providers::ProviderError> for error::Error {
    fn from(e: starknet::providers::ProviderError) -> Self {
        Self::KatanaClient(format!("Katana client RPC provider error: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        prover::{prove, state_diff::EXAMPLE_STATE_DIFF, ProverIdentifier},
        verifier::{verify, VerifierIdentifier},
    };

    #[tokio::test]
    async fn test_herodotus_verify() {
        let proof = prove(EXAMPLE_STATE_DIFF.into(), ProverIdentifier::Stone).await.unwrap();
        // std::fs::File::create("proof.json").unwrap().write_all(proof.as_bytes()).unwrap();

        let result = verify(proof, VerifierIdentifier::HerodotusStarknetSepolia).await.unwrap();

        println!("Tx: {:?}", result);
    }

    #[tokio::test]
    async fn test_local_verify() {
        let proof = prove(EXAMPLE_STATE_DIFF.into(), ProverIdentifier::Stone).await.unwrap();
        // std::fs::File::create("proof.json").unwrap().write_all(proof.as_bytes()).unwrap();

        let result = verify(proof, VerifierIdentifier::LocalStoneVerify).await.unwrap();

        println!("Tx: {:?}", result);
    }
}
