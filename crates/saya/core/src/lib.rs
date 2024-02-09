//! Saya core library.
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;

use blockifier::state::cached_state::CachedState;
use katana_primitives::contract::ClassHash;
use katana_primitives::block::BlockIdOrTag;
use katana_executor::blockifier::state::StateRefDb;
use snos::{SnOsRunner, state::SharedState, state::storage::TrieStorage};
use starknet::core::types::{BlockId, MaybePendingStateUpdate, MaybePendingBlockWithTxs, StateUpdate, ContractClass, DeclaredClassItem};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tracing::{error, trace};
use url::Url;

use crate::blockchain::Blockchain;
use crate::data_availability::{DataAvailabilityClient, DataAvailabilityConfig};
use crate::error::SayaResult;

pub mod blockchain;
pub mod data_availability;
pub mod error;
pub mod prover;
pub mod verifier;

/// Saya's main configuration.
pub struct SayaConfig {
    pub katana_rpc: Url,
    pub start_block: u64,
    pub data_availability: Option<DataAvailabilityConfig>,
}

/// Saya.
pub struct Saya {
    /// The main Saya configuration.
    config: SayaConfig,
    /// The data availability client.
    da_client: Option<Box<dyn DataAvailabilityClient>>,
    /// The katana (for now JSON RPC) client.
    katana_client: Arc<JsonRpcClient<HttpTransport>>,
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
        let katana_client =
            Arc::new(JsonRpcClient::new(HttpTransport::new(config.katana_rpc.clone())));

        let da_client = if let Some(da_conf) = &config.data_availability {
            Some(data_availability::client_from_config(da_conf.clone()).await?)
        } else {
            None
        };

        let blockchain = Blockchain::new();

        Ok(Self { config, da_client, katana_client, blockchain })
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
            let latest_block = match self.katana_client.block_number().await {
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
    async fn process_block(&mut self, block_number: u64) -> SayaResult<()> {
        trace!(block_number, "processing block");

        let state_update =
            match self.katana_client.get_state_update(BlockId::Number(block_number)).await? {
                MaybePendingStateUpdate::Update(su) => su,
                MaybePendingStateUpdate::PendingUpdate(_) => {
                    panic!("PendingUpdate should not be fetched")
                }
            };

        if block_number == 0 {
            trace!("initializing state from genesis block state diff");
            self.blockchain.init_from_state_diff(&state_update.state_diff)?;
        }

        // Fetch all decl contract classes.
        // Classes are not included in the declare transactions.
        // TODO: opti in Katana -> fetch all classes for a list of hashes instead
        // of fetching each?
        let mut contract_classes: HashMap<ClassHash, ContractClass> = HashMap::new();

        for decl in &state_update.state_diff.declared_classes {
            let DeclaredClassItem { class_hash, .. } = decl;

            let contract_class = self.katana_client.get_class(BlockId::Number(block_number), class_hash).await?;

            contract_classes.insert(*class_hash, contract_class);
        }

        self.blockchain.set_contract_classes(&contract_classes)?;

        let block_with_txs =
            match self.katana_client.get_block_with_txs(BlockId::Number(block_number)).await? {
                MaybePendingBlockWithTxs::Block(b) => b,
                MaybePendingBlockWithTxs::PendingBlock(_) => {
                    panic!("PendingBlock should not be fetched")
                }
            };

        // TODO: need to insert a block into the storage to be able to retrieve it.
        // So instead of registering manually -> use https://github.com/dojoengine/dojo/blob/c839363c5f561873355ca84da1173352f9955957/crates/katana/storage/provider/src/providers/in_memory/mod.rs#L409.
        //
        // This will update the storage automatically. But we need to populate also the fetched
        // classes. So pass the state diff + the fetched classes to construct the primitive types.
        // WE ALSO NEED THE TXNS! So instead of registring, pass all that to the function
        // and the conversions will be done in there.

        // Then Convert TXNs + block data into SNOS Input.
        // Add a module with SNOS compat.

        // To have the TXN execution info -> Need to be done on a copy of the state...?!
        // or at least we don't update the state if possible.

        // Take logic from here: https://github.com/dojoengine/dojo/blob/c839363c5f561873355ca84da1173352f9955957/crates/katana/core/src/service/block_producer.rs#L342
        // And don't register the state when getting txs output.
        // But then, when the block is processed -> we can apply the state to increment
        // the block and ensure we have the block - 1 for the next block.

        // RUN!

        trace!(block_number, txs_count = block_with_txs.transactions.len(), "block fetched");

        if block_number == 0 {
            return Ok(());
        }

        // Convert all txs into InternalTransation and write them into the file with
        // other input fields -> SNOS INPUTs file.

        // If txns -> execute txns against the state to have the execution info.
        // run SNOS.

        let snos = SnOsRunner::with_input_path("/tmp");
        let state_reader = StateRefDb::from(self.blockchain.state(&BlockIdOrTag::Number(block_number - 1))?);

        let state = SharedState {
            cache: CachedState::from(state_reader),
            block_context: self.blockchain.block_context_default(),
            commitment_storage: TrieStorage::default(),
            contract_storage: TrieStorage::default(),
            class_storage: TrieStorage::default(),
        };
        
        snos.run(state, vec![])?;

        Ok(())
    }

    /// Fetches the state update for the given block and publish it to
    /// the data availability layer (if any).
    /// Returns the [`StateUpdate`].
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number to get state update for.
    async fn fetch_publish_state_update(&self, block_number: u64) -> SayaResult<StateUpdate> {
        let state_update =
            match self.katana_client.get_state_update(BlockId::Number(block_number)).await? {
                MaybePendingStateUpdate::Update(su) => {
                    if let Some(da) = &self.da_client {
                        let sd_felts =
                            data_availability::state_diff::state_diff_to_felts(&su.state_diff);

                        da.publish_state_diff_felts(&sd_felts).await?;
                    }

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
