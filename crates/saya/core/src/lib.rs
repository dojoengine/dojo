//! Saya core library.

use std::sync::Arc;

use blockifier::block_context::{BlockContext, BlockInfo, ChainInfo, FeeTokenAddresses, GasPrices};
use blockifier::state::cached_state::CachedState;
use katana_executor::blockifier::state::StateRefDb;
use katana_primitives::block::{BlockIdOrTag, BlockNumber, FinalityStatus, SealedBlockWithStatus};
use katana_primitives::chain::ChainId;
use saya_provider::rpc::JsonRpcProvider;
use saya_provider::Provider as SayaProvider;
use snos::state::storage::TrieStorage;
use snos::state::SharedState;
use snos::SnOsRunner;
use serde::{Deserialize, Serialize};
use tracing::{error, trace};
use url::Url;

use crate::blockchain::Blockchain;
use crate::data_availability::{DataAvailabilityClient, DataAvailabilityConfig};
use crate::error::SayaResult;

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

        let block = self.provider.fetch_block(block_number).await?;
        let state_updates = self.provider.fetch_state_updates(block_number).await?;

        let block = SealedBlockWithStatus {
            block,
            // If the block is fetched, it's because it is not yet proven.
            status: FinalityStatus::AcceptedOnL2,
        };

        self.blockchain.update_state_with_block(block.clone(), state_updates)?;

        if block_number == 0 {
            return Ok(());
        }

        // TODO: all this must come from katana if we keep execution here.
        // But this should not be the case and all the execution info
        // should be fetched from Katana.
        let invoke_tx_max_n_steps = 100000000;
        let validate_max_n_steps = 100000000;
        let chain_id = ChainId::parse("KATANA").unwrap();
        let fee_token_addresses = FeeTokenAddresses {
            eth_fee_token_address: 0_u128.into(),
            strk_fee_token_address: 0_u128.into(),
        };

        let block_info = blockchain::block_info_from_header(
            &block.block.header,
            invoke_tx_max_n_steps,
            validate_max_n_steps,
        );

        let block_context = BlockContext {
            block_info,
            chain_info: ChainInfo { fee_token_addresses, chain_id: chain_id.into() },
        };

        // TODO: fetch this from a new katana endpoints when
        // katana stored [`TransactionExecutionInfo`].
        let exec_infos = self.blockchain.execute_transactions(&block.block, &block_context)?;

        let input_path = std::path::PathBuf::from("/tmp/input.json");
        let snos_input = crate::starknet_os::input::snos_input_from_block(&block.block);
        snos_input.dump(&input_path)?;

        let snos = SnOsRunner {
            layout: String::from("starknet_with_keccak"),
            os_path: String::from("/tmp/os.json"),
            input_path: input_path.to_string_lossy().into(),
            block_context: block_context.clone(),
        };

        let state_reader =
            StateRefDb::from(self.blockchain.state(&BlockIdOrTag::Number(block_number - 1))?);

        let state = SharedState {
            cache: CachedState::from(state_reader),
            block_context,
            commitment_storage: TrieStorage::default(),
            contract_storage: TrieStorage::default(),
            class_storage: TrieStorage::default(),
        };

        snos.run(state, vec![])?;

        Ok(())
    }

    // Fetches the state update for the given block and publish it to
    // the data availability layer (if any).
    // Returns the [`StateUpdate`].
    //
    // # Arguments
    //
    // * `block_number` - The block number to get state update for.
    // async fn fetch_publish_state_update(&self, block_number: u64) -> SayaResult<StateUpdate> {
    // let state_update =
    // match self.katana_client.get_state_update(BlockId::Number(block_number)).await? {
    // MaybePendingStateUpdate::Update(su) => {
    // if let Some(da) = &self.da_client {
    // let sd_felts =
    // data_availability::state_diff::state_diff_to_felts(&su.state_diff);
    //
    // da.publish_state_diff_felts(&sd_felts).await?;
    // }
    //
    // su
    // }
    // MaybePendingStateUpdate::PendingUpdate(_) => unreachable!("Should not be used"),
    // };
    //
    // Ok(state_update)
    // }
}

impl From<starknet::providers::ProviderError> for error::Error {
    fn from(e: starknet::providers::ProviderError) -> Self {
        Self::KatanaClient(format!("Katana client RPC provider error: {e}"))
    }
}
