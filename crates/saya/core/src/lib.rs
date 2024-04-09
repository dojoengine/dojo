//! Saya core library.

use std::sync::Arc;

use futures::future::join;
use katana_primitives::block::{BlockNumber, FinalityStatus, SealedBlock, SealedBlockWithStatus};
use katana_primitives::transaction::Tx;
use katana_primitives::FieldElement;
use prover::ProverIdentifier;
use saya_provider::rpc::JsonRpcProvider;
use saya_provider::Provider as SayaProvider;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::{error, info, trace};
use url::Url;
use verifier::VerifierIdentifier;

use crate::blockchain::Blockchain;
use crate::data_availability::{DataAvailabilityClient, DataAvailabilityConfig};
use crate::error::SayaResult;
use crate::prover::{extract_messages, ProgramInput};

pub mod blockchain;
pub mod data_availability;
pub mod error;
pub mod prover;
pub mod starknet_os;
pub mod verifier;

pub(crate) const LOG_TARGET: &str = "saya::core";

/// Saya's main configuration.
#[derive(Debug, Deserialize, Serialize)]
pub struct SayaConfig {
    #[serde(deserialize_with = "url_deserializer")]
    pub katana_rpc: Url,
    pub start_block: u64,
    pub data_availability: Option<DataAvailabilityConfig>,
    pub prover: ProverIdentifier,
    pub verifier: VerifierIdentifier,
    pub world_address: Option<FieldElement>,
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
        let mut block = self.config.start_block.max(1); // Genesis block is not proven. We advance to block 1

        let (genesis_block, block_before_the_first) =
            join(self.provider.fetch_block(0), self.provider.fetch_block(block - 1)).await;
        let genesis_state_hash = genesis_block?.header.header.state_root;
        let mut previous_block = block_before_the_first?;

        loop {
            let latest_block = match self.provider.block_number().await {
                Ok(block_number) => block_number,
                Err(e) => {
                    error!(target: LOG_TARGET, error = ?e, "Fetching block.");
                    tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                    continue;
                }
            };

            if block > latest_block {
                trace!(target: LOG_TARGET, block_number = block, "Waiting for block.");
                tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                continue;
            }

            let fetched_block = self.provider.fetch_block(block).await?;

            self.process_block(block, (&fetched_block, previous_block, genesis_state_hash)).await?;

            previous_block = fetched_block;
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
    async fn process_block(
        &mut self,
        block_number: BlockNumber,
        blocks: (&SealedBlock, SealedBlock, FieldElement),
    ) -> SayaResult<()> {
        trace!(target: LOG_TARGET, block_number = %block_number, "Processing block.");

        let (block, prev_block, _genesis_state_hash) = blocks;

        let (state_updates, da_state_update) =
            self.provider.fetch_state_updates(block_number).await?;

        if let Some(da) = &self.da_client {
            da.publish_state_diff_felts(&da_state_update).await?;
        }

        let block =
            SealedBlockWithStatus { block: block.clone(), status: FinalityStatus::AcceptedOnL2 };

        let state_updates_to_prove = state_updates.state_updates.clone();
        self.blockchain.update_state_with_block(block.clone(), state_updates)?;

        if block_number == 0 {
            return Ok(());
        }

        let exec_infos = self.provider.fetch_transactions_executions(block_number).await?;

        if exec_infos.is_empty() {
            trace!(target: "saya_core", block_number, "Skipping empty block.");
            return Ok(());
        }

        let transactions = block
            .block
            .body
            .iter()
            .filter_map(|t| match &t.transaction {
                Tx::L1Handler(tx) => Some(tx),
                _ => None,
            })
            .collect::<Vec<_>>();

        let (message_to_starknet_segment, message_to_appchain_segment) =
            extract_messages(&exec_infos, transactions);

        let new_program_input = ProgramInput {
            prev_state_root: prev_block.header.header.state_root,
            block_number: FieldElement::from(block_number),
            block_hash: block.block.header.hash,
            config_hash: FieldElement::from(0u64),
            message_to_starknet_segment,
            message_to_appchain_segment,
            state_updates: state_updates_to_prove,
        };

        println!("Program input: {}", new_program_input.serialize(self.config.world_address)?);

        // let to_prove = ProvedStateDiff {
        //     genesis_state_hash,
        //     prev_state_hash: prev_block.header.header.state_root,
        //     state_updates: state_updates_to_prove,
        // };

        trace!(target: "saya_core", "Proving block {block_number}.");
        let proof = prover::prove(
            new_program_input.serialize(self.config.world_address)?,
            self.config.prover,
        )
        .await?;
        info!(target: "saya_core", block_number, "Block proven.");

        // save proof to file
        tokio::fs::File::create(format!("proof_{}.json", block_number))
            .await
            .unwrap()
            .write_all(proof.as_bytes())
            .await
            .unwrap();

        trace!(target: "saya_core", "Verifying block {block_number}.");
        let transaction_hash = verifier::verify(proof, self.config.verifier).await?;
        info!(target: "saya_core", block_number, transaction_hash, "Block verified.");

        Ok(())
    }
}

impl From<starknet::providers::ProviderError> for error::Error {
    fn from(e: starknet::providers::ProviderError) -> Self {
        Self::KatanaClient(format!("Katana client RPC provider error: {e}"))
    }
}

// CI is not allowing to fetch images from inside the docker itself.
// Need to be addressed, so tests by pulling prover and verifier are for now
// disabled here, but can be uncommented to test locally.
// #[cfg(test)]
// mod tests {
//     use crate::prover::state_diff::EXAMPLE_STATE_DIFF;
//     use crate::prover::{prove, ProverIdentifier};
//     use crate::verifier::{verify, VerifierIdentifier};

//     #[tokio::test]
//     async fn test_herodotus_verify() {
//         let proof = prove(EXAMPLE_STATE_DIFF.into(), ProverIdentifier::Stone).await.unwrap();
//         let _tx = verify(proof, VerifierIdentifier::HerodotusStarknetSepolia).await.unwrap();
//     }

//     #[tokio::test]
//     async fn test_local_verify() {
//         let proof = prove(EXAMPLE_STATE_DIFF.into(), ProverIdentifier::Stone).await.unwrap();
//         let _res = verify(proof, VerifierIdentifier::StoneLocal).await.unwrap();
//     }
// }
