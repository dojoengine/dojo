//! Saya core library.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::convert::identity;
use std::ops::RangeInclusive;
use std::sync::Arc;

use anyhow::Context;
use cairo_proof_parser::output::ExtractOutputResult;
use cairo_proof_parser::{to_felts, StarkProof};
use dojo_os::piltover::starknet_apply_piltover;
use futures::future;
use katana_primitives::block::{BlockNumber, FinalityStatus, SealedBlock, SealedBlockWithStatus};
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::Tx;
use katana_primitives::FieldElement;
use katana_rpc_types::trace::TxExecutionInfo;
use prover::{
    extract_execute_calls, HttpProverParams, ProofAndDiff, ProveDiffProgram, ProveProgram,
    ProverIdentifier,
};
pub use prover_sdk::ProverAccessKey;
use saya_provider::rpc::JsonRpcProvider;
use saya_provider::Provider as SayaProvider;
use serde::{Deserialize, Serialize};
use starknet::accounts::{Call, ConnectedAccount};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::felt;
use starknet_crypto::poseidon_hash_many;
use starknet_types_core::felt::Felt;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, trace};
use url::Url;
use verifier::VerifierIdentifier;

use crate::blockchain::Blockchain;
use crate::data_availability::{DataAvailabilityClient, DataAvailabilityConfig};
use crate::error::SayaResult;
use crate::prover::{extract_messages, ProgramInput, Scheduler};

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
    pub prover_url: Url,
    pub prover_key: ProverAccessKey,
    pub store_proofs: bool,
    pub block_range: (u64, Option<u64>),
    pub batch_size: usize,
    pub data_availability: Option<DataAvailabilityConfig>,
    pub world_address: FieldElement,
    pub fact_registry_address: FieldElement,
    pub skip_publishing_proof: bool,
    pub starknet_account: StarknetAccountData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StarknetAccountData {
    #[serde(deserialize_with = "url_deserializer")]
    pub starknet_url: Url,
    #[serde(deserialize_with = "felt_string_deserializer")]
    pub chain_id: FieldElement,
    pub signer_address: FieldElement,
    pub signer_key: FieldElement,
}

pub fn url_deserializer<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Url::parse(&s).map_err(serde::de::Error::custom)
}

pub fn felt_string_deserializer<'de, D>(deserializer: D) -> Result<FieldElement, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    cairo_short_string_to_felt(&s).map_err(serde::de::Error::custom)
}

/// Saya.
#[allow(missing_debug_implementations)]
pub struct Saya {
    /// The main Saya configuration.
    config: SayaConfig,
    /// The data availability client.
    da_client: Option<Box<dyn DataAvailabilityClient>>,
    /// The provider to fetch dojo from Katana.
    provider: Arc<dyn SayaProvider>,
    /// The blockchain state.
    blockchain: Blockchain,
    /// The proving backend identifier.
    prover_identifier: ProverIdentifier,
}

struct FetchedBlockInfo {
    block_number: BlockNumber,
    block: SealedBlock,
    prev_state_root: FieldElement,
    state_updates: StateUpdatesWithDeclaredClasses,
    exec_infos: Vec<TxExecutionInfo>,
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

        let prover_identifier = ProverIdentifier::Http(Arc::new(HttpProverParams {
            prover_url: config.prover_url.clone(),
            prover_key: config.prover_key.clone(),
        }));

        Ok(Self { config, da_client, provider, blockchain, prover_identifier })
    }

    /// Starts the Saya mainloop to fetch and process data.
    ///
    /// Optims:
    /// First naive version to have an overview of all the components
    /// and the process.
    /// Should be refacto in crates as necessary.
    pub async fn start(&mut self) -> SayaResult<()> {
        let poll_interval_secs = 1;

        let mut block = self.config.block_range.0.max(1); // Genesis block is not proven. We advance to block 1

        let block_before_the_first = self.provider.fetch_block(block - 1).await;
        let mut previous_block_state_root = block_before_the_first?.header.header.state_root;

        let prover_identifier = ProverIdentifier::Http(Arc::new(HttpProverParams {
            prover_url: self.config.prover_url.clone(),
            prover_key: self.config.prover_key.clone(),
        }));

        // The structure responsible for proving.
        let mut prove_scheduler = None;

        loop {
            let latest_block = match self.provider.block_number().await {
                Ok(block_number) => block_number,
                Err(e) => {
                    error!(target: LOG_TARGET, error = ?e, "Fetching block.");
                    tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                    continue;
                }
            };

            let (minimum_expected, maximum_expected) = match self.config.mode {
                SayaMode::Ephemeral => {
                    let last = self.config.block_range.1.unwrap_or(block);
                    (last, last) // Only one proof is generated, no need to fetch earlier.
                }
                SayaMode::PersistentMerging => (block, latest_block), // Separate proofs for each block, starting as early as possible.
                SayaMode::Persistent => (block, latest_block), // One proof per batch, waiting until all are available.
            };

            if minimum_expected > latest_block {
                trace!(target: LOG_TARGET, block_number = latest_block + 1, "Waiting for block.");
                tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval_secs)).await;
                continue;
            }

            let (last_state_root, params) =
                self.prefetch_blocks(block..=maximum_expected, previous_block_state_root).await?;
            previous_block_state_root = last_state_root;

            // Updating the local state sequentially, as there is only one instance of
            // `self.blockchain` This part does no actual  proving, so should not be a
            // problem

            match self.config.mode {
                SayaMode::PersistentMerging => {
                    for p in params {
                        let mut scheduler = if let Some(scheduler) = prove_scheduler {
                            prove_scheduler = None;
                            scheduler
                        } else {
                            Scheduler::new(
                                self.config.batch_size,
                                self.config.world_address,
                                self.prover_identifier.clone(),
                            )
                        };

                        if let Some((prover_input, _)) = self.process_block(block, p)? {
                            scheduler.push_diff(prover_input)?;
                            if scheduler.is_full() {
                                self.process_proven(scheduler.proved().await?).await?;
                            } else {
                                prove_scheduler = Some(scheduler);
                            }
                        }

                        block += 1;
                    }
                }

                SayaMode::Persistent => {
                    let num_blocks = params.len() as u64;
                    let calls = params
                        .into_iter()
                        .enumerate()
                        .map(|(i, p)| self.process_block(block + i as u64, p))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .filter_map(identity)
                        .map(|(_, c)| c)
                        .flatten()
                        .collect::<Vec<_>>();

                    // We might want to prove the signatures as well.
                    let proof = self.prover_identifier.prove_snos(calls).await?;

                    if self.config.store_proofs {
                        let filename = format!("proof_{}.json", block + num_blocks - 1);
                        let mut file =
                            File::create(filename).await.context("Failed to create proof file.")?;
                        file.write_all(proof.as_bytes()).await.context("Failed to write proof.")?;
                    }

                    let proof = StarkProof::try_from(proof.as_str())?;
                    let program_output = proof.extract_output()?.program_output;

                    self.process_proven((proof, dbg!(program_output), (block, block + num_blocks)))
                        .await?;

                    block += num_blocks;
                    info!(target: LOG_TARGET, "Successfully processed {} blocks.", num_blocks);
                }

                SayaMode::Ephemeral => {
                    let num_blocks = params.len() as u64;
                    let calls = params
                        .into_iter()
                        .enumerate()
                        .map(|(i, p)| self.process_block(block + i as u64, p))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .filter_map(identity)
                        .map(|(_, c)| c)
                        .flatten()
                        .collect::<Vec<_>>();

                    // We might want to prove the signatures as well.
                    let proof = self.prover_identifier.prove_checker(calls).await?;

                    if self.config.store_proofs {
                        let filename = format!("proof_{}.json", block + num_blocks - 1);
                        let mut file =
                            File::create(filename).await.context("Failed to create proof file.")?;
                        file.write_all(proof.as_bytes()).await.context("Failed to write proof.")?;
                    }

                    let block_range =
                        (self.config.block_range.0, self.config.block_range.1.unwrap());

                    let proof = StarkProof::try_from(proof.as_str())?;
                    let diff = proof.extract_output()?.program_output;
                    self.process_proven((proof, dbg!(diff), block_range)).await?;

                    info!(target: LOG_TARGET, "Successfully processed all {} blocks.", block_range.1 - block_range.0 + 1);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn prefetch_blocks(
        &mut self,
        block_numbers: RangeInclusive<BlockNumber>,
        previous_block_state_root: FieldElement,
    ) -> SayaResult<(FieldElement, Vec<FetchedBlockInfo>)> {
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

        // The serialized DA is not used here as we only need the state updates to generate the
        // proof and the DA data are generated by the `dojo-os`.
        let (state_updates, _): (Vec<_>, Vec<_>) = future::try_join_all(
            block_numbers
                .clone()
                .map(|block_number| self.provider.fetch_state_updates(block_number)),
        )
        .await?
        .into_iter()
        .unzip();
        let transactions_executions = future::try_join_all(
            block_numbers
                .clone()
                .map(|block_number| self.provider.fetch_transactions_executions(block_number)),
        )
        .await?;

        state_updates.into_iter().zip(transactions_executions.into_iter()).for_each(
            |(state_updates, exec_info)| {
                state_updates_and_exec_info.push((state_updates, exec_info));
            },
        );

        // Prepare parameters
        let params = fetched_blocks
            .into_iter()
            .zip(state_roots)
            .zip(state_updates_and_exec_info)
            .map(|((block, prev_state_root), (state_updates, exec_infos))| FetchedBlockInfo {
                block_number: block.header.header.number,
                block,
                prev_state_root,
                state_updates,
                exec_infos,
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
    /// * `block_info` - The block to process, along with the state roots of the previous block and
    ///   the genesis block.
    fn process_block(
        &mut self,
        block_number: BlockNumber,
        block_info: FetchedBlockInfo,
    ) -> SayaResult<Option<(ProgramInput, Vec<Call>)>> {
        trace!(target: LOG_TARGET, block_number = %block_number, "Processing block.");

        let FetchedBlockInfo { block, prev_state_root, state_updates, exec_infos, block_number } =
            block_info;

        let block =
            SealedBlockWithStatus { block: block.clone(), status: FinalityStatus::AcceptedOnL2 };

        let state_updates_to_prove = state_updates.state_updates.clone();
        self.blockchain.update_state_with_block(block.clone(), state_updates)?;

        if block_number == 0 {
            return Ok(None);
        }

        if exec_infos.is_empty() {
            trace!(target: LOG_TARGET, block_number, "Skipping empty block.");
            return Ok(None);
        }

        let l1_transactions = block
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
            extract_messages(&exec_infos, &l1_transactions);

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

        info!(target: LOG_TARGET, block_number, "Block processed.");

        let calls = extract_execute_calls(&exec_infos);

        Ok(Some((state_diff_prover_input, calls)))
    }

    /// Registers the facts + the send the proof to verifier. Not all provers require this step
    ///    (a.k.a. SHARP).
    ///
    /// # Arguments
    ///
    /// * `prove_scheduler` - A full parallel prove scheduler.
    /// * `last_block` - The last block number in the `prove_scheduler`.
    async fn process_proven(&self, proven: ProofAndDiff) -> SayaResult<()> {
        // Prove each of the leaf nodes of the recursion tree and merge them into one
        let (proof, state_diff, (_, last_block)) = proven;

        trace!(target: LOG_TARGET, last_block, "Processing proven blocks.");

        let serialized_proof = to_felts(&proof).context("Failed to serialize proof.")?;

        // Publish state difference if DA client is available.
        if let Some(da) = &self.da_client {
            trace!(target: LOG_TARGET, last_block, "Publishing DA.");

            if self.config.skip_publishing_proof {
                da.publish_state_diff_felts(&state_diff).await?;
            } else {
                da.publish_state_diff_and_proof_felts(&state_diff, &serialized_proof).await?;
            }
        }

        let program_hash = proof.extract_program()?.program_hash;
        let ExtractOutputResult { program_output, program_output_hash } = proof.extract_output()?;
        let expected_fact = poseidon_hash_many(&[program_hash, program_output_hash]).to_string();
        let program = program_hash.to_string();
        info!(target: LOG_TARGET, expected_fact, program, "Expected fact.");

        // Verify the proof and register fact.
        trace!(target: LOG_TARGET, last_block, "Verifying block.");
        let (transaction_hash, nonce_after) = verifier::verify(
            VerifierIdentifier::HerodotusStarknetSepolia(self.config.fact_registry_address),
            serialized_proof,
            self.config.starknet_account.clone(),
            self.config.mode.to_program().cairo_version(),
        )
        .await?;
        info!(target: LOG_TARGET, last_block, transaction_hash, "Block verified.");

        // Apply the diffs to the world state.
        match self.config.mode {
            SayaMode::Ephemeral => {
                // Needs checker program to be verified, and set as the upgrade_state authority
                todo!("Ephemeral mode does not support publishing updated state yet.");
            }
            SayaMode::Persistent => {
                let piltover_contract =
                    felt!("0x2e488ebbcfc05d7129a8aa8f3e8853a4413c1cfc153f03ca18cc317ebdb50e0");

                dbg!(state_diff);

                starknet_apply_piltover(piltover_contract, nonce_after + 1u64.into()).await?;
            }
            SayaMode::PersistentMerging => {
                // When not waiting for couple of second `apply_diffs` will sometimes fail due to reliance
                // on registered fact
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                trace!(target: LOG_TARGET, last_block, "Applying diffs.");
                let transaction_hash = dojo_os::starknet_apply_diffs(
                    self.config.world_address,
                    state_diff,
                    program_output,
                    program_hash,
                    nonce_after + 1u64.into(),
                    self.config.starknet_account,
                )
                .await?;
                info!(target: LOG_TARGET, last_block, transaction_hash, "Diffs applied.");
            }
        }

        Ok(())
    }
}

impl From<starknet::providers::ProviderError> for error::Error {
    fn from(e: starknet::providers::ProviderError) -> Self {
        Self::KatanaClient(format!("Katana client RPC provider error: {e}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SayaMode {
    Ephemeral,
    Persistent,
    PersistentMerging,
}

impl SayaMode {
    fn to_program(&self) -> ProveProgram {
        match self {
            SayaMode::Ephemeral => ProveProgram::Checker,
            SayaMode::Persistent => ProveProgram::Batcher,
            SayaMode::PersistentMerging => ProveProgram::DiffProgram(ProveDiffProgram::Merger),
        }
    }
}
