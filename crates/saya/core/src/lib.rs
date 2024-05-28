//! Saya core library.

use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use cairo_proof_parser::output::{extract_output, ExtractOutputResult};
use cairo_proof_parser::parse;
use cairo_proof_parser::program::{extract_program, ExtractProgramResult};
use futures::future;
use katana_primitives::block::{BlockNumber, FinalityStatus, SealedBlock, SealedBlockWithStatus};
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::Tx;
use katana_primitives::FieldElement;
use prover::{HttpProverParams, ProverIdentifier};
pub use prover_sdk::ProverAccessKey;
use saya_provider::rpc::JsonRpcProvider;
use saya_provider::Provider as SayaProvider;
use serde::{Deserialize, Serialize};
use starknet_crypto::poseidon_hash_many;
use tokio::time::sleep;
use tracing::{error, info, trace};
use url::Url;

use crate::blockchain::Blockchain;
use crate::data_availability::{DataAvailabilityClient, DataAvailabilityConfig};
use crate::error::SayaResult;
use crate::prover::{extract_messages, ProgramInput, Scheduler};
use crate::verifier::VerifierIdentifier;

pub mod blockchain;
pub mod data_availability;
pub mod dojo_os;
pub mod error;
pub mod prover;
pub mod verifier;

pub(crate) const LOG_TARGET: &str = "saya::core";

/// Saya's main configuration.
#[derive(Debug, Deserialize, Serialize)]
pub struct SayaConfig {
    #[serde(deserialize_with = "url_deserializer")]
    pub katana_rpc: Url,
    #[serde(deserialize_with = "url_deserializer")]
    pub url: Url,
    pub private_key: ProverAccessKey,
    pub start_block: u64,
    pub batch_size: usize,
    pub data_availability: Option<DataAvailabilityConfig>,
    pub world_address: FieldElement,
    pub fact_registry_address: FieldElement,
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

        let block_before_the_first = self.provider.fetch_block(block - 1).await;
        let mut previous_block_state_root = block_before_the_first?.header.header.state_root;

        let prover_identifier = ProverIdentifier::Http(Arc::new(HttpProverParams {
            prover_url: self.config.url.clone(),
            prover_key: self.config.private_key.clone(),
        }));

        // The structure responsible for proving.
        let mut prove_scheduler = Scheduler::new(
            self.config.batch_size,
            self.config.world_address,
            prover_identifier.clone(),
        );

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

            let (last_state_root, params) =
                self.prefetch_blocks(block..=latest_block, previous_block_state_root).await?;
            previous_block_state_root = last_state_root;

            // Updating the local state sequentially, as there is only one instance of
            // `self.blockchain` This part does no actual  proving, so should not be a
            // problem
            for p in params.clone() {
                self.process_block(&mut prove_scheduler, block, p)?;

                if prove_scheduler.is_full() {
                    self.process_proven(prove_scheduler, block).await?;

                    prove_scheduler = Scheduler::new(
                        self.config.batch_size,
                        self.config.world_address,
                        prover_identifier.clone(),
                    );
                }

                block += 1;
            }
        }
    }

    async fn prefetch_blocks(
        &mut self,
        block_numbers: RangeInclusive<BlockNumber>,
        previous_block_state_root: FieldElement,
    ) -> SayaResult<(
        FieldElement,
        Vec<(SealedBlock, FieldElement, StateUpdatesWithDeclaredClasses, Vec<TxExecInfo>)>,
    )> {
        // Fetch all blocks from the current block to the latest block
        let fetched_blocks = future::try_join_all(
            block_numbers.clone().map(|block_number| self.provider.fetch_block(block_number)),
        )
        .await?;

        // Shift the state roots to the right by one, as proof of each block is based on the
        // previous state root
        let mut state_roots = vec![previous_block_state_root];
        state_roots.extend(fetched_blocks.iter().map(|block| block.header.header.state_root));
        let previous_block_state_root = state_roots.pop().unwrap();

        let mut state_updates_and_exec_info = vec![];

        for block in block_numbers.clone() {
            let (state_updates, da_state_update) = self.provider.fetch_state_updates(block).await?;
            if let Some(da) = &self.da_client {
                // Publish state difference if DA client is available
                da.publish_state_diff_felts(&da_state_update).await?;
            }
            let exec_infos = self.provider.fetch_transactions_executions(block).await?;
            state_updates_and_exec_info.push((state_updates, exec_infos));
        }

        // Prepare parameters
        let params = fetched_blocks
            .into_iter()
            .zip(state_roots)
            .zip(state_updates_and_exec_info)
            .map(|((block, state_root), (state_updates, exec_infos))| {
                (block, state_root, state_updates, exec_infos)
            })
            .collect::<Vec<_>>();

        trace!(target: LOG_TARGET, block_number = block_numbers.start(), to = block_numbers.end(), "Fetched blocks.");

        Ok((previous_block_state_root, params))
    }

    /// Processes the given block number.
    ///
    /// # Summary
    ///
    /// 1. Update local state accordingly to pulled state. We may publish DA at this point.
    ///
    /// 2. Pulls all transactions and data required to generate the trace.
    ///
    /// 3. Computes facts for this state transition. We may optimistically register the facts.
    ///
    /// 4. Starts computing the proof from the trace with a prover.
    ///
    /// # Arguments
    ///
    /// * `prove_scheduler` - A parallel prove scheduler.
    /// * `block_number` - The block number.
    /// * `blocks` - The block to process, along with the state roots of the previous block and the
    ///   genesis block.
    fn process_block(
        &mut self,
        prove_scheduler: &mut Scheduler,
        block_number: BlockNumber,
        blocks: (SealedBlock, FieldElement, StateUpdatesWithDeclaredClasses, Vec<TxExecInfo>),
    ) -> SayaResult<()> {
        trace!(target: LOG_TARGET, block_number = %block_number, "Processing block.");

        let (block, prev_state_root, state_updates, exec_infos) = blocks;

        let block =
            SealedBlockWithStatus { block: block.clone(), status: FinalityStatus::AcceptedOnL2 };

        let state_updates_to_prove = state_updates.state_updates.clone();
        self.blockchain.update_state_with_block(block.clone(), state_updates)?;

        if block_number == 0 {
            return Ok(());
        }

        if exec_infos.is_empty() {
            trace!(target: LOG_TARGET, block_number, "Skipping empty block.");
            return Ok(());
        }

        let transactions = block
            .block
            .body
            .iter()
            .filter_map(|t| match &t.transaction {
                // attach the tx hash for filtering when extracting messages later
                Tx::L1Handler(tx) => Some((t.hash, tx)),
                _ => None,
            })
            .collect::<Vec<_>>();

        let (message_to_starknet_segment, message_to_appchain_segment) =
            extract_messages(&exec_infos, &transactions);

        let mut state_diff_prover_input = ProgramInput {
            prev_state_root,
            block_number,
            block_hash: block.block.header.hash,
            config_hash: FieldElement::from(0u64),
            message_to_starknet_segment,
            message_to_appchain_segment,
            state_updates: state_updates_to_prove,
            world_da: None,
        };
        state_diff_prover_input.fill_da(self.config.world_address);

        prove_scheduler.push_diff(state_diff_prover_input).unwrap();

        info!(target: LOG_TARGET, block_number, "Block processed.");

        Ok(())
    }

    /// Registers the facts + the send the proof to verifier. Not all provers require this step
    ///    (a.k.a. SHARP).
    ///
    /// # Arguments
    ///
    /// * `prove_scheduler` - A full parallel prove scheduler.
    /// * `last_block` - The last block number in the `prove_scheduler`.
    async fn process_proven(
        &self,
        prove_scheduler: Scheduler,
        last_block: BlockNumber,
    ) -> SayaResult<()> {
        // Prove each of the leaf nodes of the recursion tree and merge them into one
        let (proof, state_diff) = prove_scheduler.proved().await.context("Failed to prove.")?;

        // std::io::Write::write_all(
        //     &mut std::fs::File::create("proof.json").unwrap(),
        //     proof.as_bytes(),
        // )
        // .unwrap();

        let serialized_proof: Vec<FieldElement> = parse(&proof)?.into();
        let world_da = state_diff.world_da.unwrap();

        trace!(target: LOG_TARGET, last_block, "Verifying block.");
        let transaction_hash = verifier::verify(
            VerifierIdentifier::HerodotusStarknetSepolia(self.config.fact_registry_address),
            serialized_proof,
        )
        .await?;
        info!(target: LOG_TARGET, last_block, transaction_hash, "Block verified.");

        let ExtractProgramResult { program: _, program_hash } = extract_program(&proof)?;
        println!("Program hash: {:x}", program_hash);
        let ExtractOutputResult { program_output, program_output_hash } = extract_output(&proof)?;
        let expected_fact = poseidon_hash_many(&[program_hash, program_output_hash]).to_string();
        info!(target: LOG_TARGET, expected_fact, "Expected fact.");

        sleep(Duration::from_millis(5000)).await;

        trace!(target: LOG_TARGET, last_block, "Applying diffs.");
        let transaction_hash = dojo_os::starknet_apply_diffs(
            self.config.world_address,
            world_da,
            program_output,
            program_hash,
        )
        .await?;
        info!(target: LOG_TARGET, last_block, transaction_hash, "Diffs applied.");

        Ok(())
    }
}

impl From<starknet::providers::ProviderError> for error::Error {
    fn from(e: starknet::providers::ProviderError) -> Self {
        Self::KatanaClient(format!("Katana client RPC provider error: {e}"))
    }
}
