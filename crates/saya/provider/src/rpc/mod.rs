//! Provider to fetch Katana data from RPC.
//!
//! The transport here is fixed to JSON RPC.
use std::sync::Arc;

use anyhow::anyhow;
use jsonrpsee::http_client::HttpClientBuilder;
use katana_primitives::block::{
    BlockIdOrTag, BlockNumber, GasPrices, Header, SealedBlock, SealedHeader,
};
use katana_primitives::chain::ChainId;
use katana_primitives::conversion::rpc as rpc_converter;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::TxWithHash;
use katana_primitives::version::ProtocolVersion;
use katana_rpc_api::saya::SayaApiClient;
use katana_rpc_types::trace::TxExecutionInfo;
use num_traits::ToPrimitive;
use starknet::core::types::{
    ContractClass, Felt, MaybePendingBlockWithTxs, MaybePendingStateUpdate,
};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider as StarknetProvider};
use tracing::trace;
use url::Url;

use crate::provider::Provider;
use crate::rpc::{state as state_converter, transaction as tx_converter};
use crate::{ProviderResult, LOG_TARGET};

mod state;
mod transaction;

/// A JSON RPC provider.
#[derive(Debug)]
pub struct JsonRpcProvider {
    /// The RPC URL that must be kept for custom endpoints.
    rpc_url: String,
    /// The Starknet provider.
    starknet_provider: Arc<JsonRpcClient<HttpTransport>>,
    /// Chain id detected from the `starknet_provider`.
    chain_id: ChainId,
}

impl JsonRpcProvider {
    /// Initializes a new [`JsonRpcProvider`].
    /// Will attempt to fetch the chain id from the provider.
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - The RPC url to fetch data from. Must be up and running to fetch the chain id.
    pub async fn new(rpc_url: Url) -> ProviderResult<Self> {
        let starknet_provider = Arc::new(JsonRpcClient::new(HttpTransport::new(rpc_url.clone())));

        let chain_id: ChainId = starknet_provider.chain_id().await?.into();

        Ok(Self { starknet_provider, chain_id, rpc_url: rpc_url.to_string() })
    }

    /// Returns the internal [`ChainId`].
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }
}

#[async_trait::async_trait]
impl Provider for JsonRpcProvider {
    async fn block_number(&self) -> ProviderResult<BlockNumber> {
        Ok(self.starknet_provider.block_number().await?)
    }

    async fn fetch_block(&self, block_number: BlockNumber) -> ProviderResult<SealedBlock> {
        let block = match self
            .starknet_provider
            .get_block_with_txs(BlockIdOrTag::Number(block_number))
            .await?
        {
            MaybePendingBlockWithTxs::Block(b) => b,
            MaybePendingBlockWithTxs::PendingBlock(_) => {
                panic!("PendingBlock should not be fetched")
            }
        };

        let txs: Vec<TxWithHash> = block
            .transactions
            .iter()
            .map(|tx_rpc| tx_converter::tx_from_rpc(tx_rpc, self.chain_id))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SealedBlock {
            header: SealedHeader {
                hash: block.block_hash,
                header: Header {
                    parent_hash: block.parent_hash,
                    number: block.block_number,
                    gas_prices: GasPrices::new(
                        block.l1_gas_price.price_in_wei.to_u128().unwrap(),
                        block.l1_gas_price.price_in_fri.to_u128().unwrap(),
                    ),
                    timestamp: block.timestamp,
                    state_root: block.new_root,
                    sequencer_address: block.sequencer_address.into(),
                    protocol_version: ProtocolVersion::parse(&block.starknet_version).unwrap(),
                },
            },
            body: txs,
        })
    }

    async fn fetch_state_updates(
        &self,
        block_number: BlockNumber,
    ) -> ProviderResult<(StateUpdatesWithDeclaredClasses, Vec<Felt>)> {
        let rpc_state_update = match self
            .starknet_provider
            .get_state_update(BlockIdOrTag::Number(block_number))
            .await?
        {
            MaybePendingStateUpdate::Update(su) => su,
            MaybePendingStateUpdate::PendingUpdate(_) => {
                return Err(anyhow!("PendingUpdate should not be fetched").into());
            }
        };

        let serialized_state_update =
            state_converter::state_diff_to_felts(&rpc_state_update.state_diff);
        let state_updates = state_converter::state_updates_from_rpc(&rpc_state_update)?;

        let mut state_updates_with_classes =
            StateUpdatesWithDeclaredClasses { state_updates, ..Default::default() };

        for class_hash in state_updates_with_classes.state_updates.declared_classes.keys() {
            match self
                .starknet_provider
                .get_class(BlockIdOrTag::Number(block_number), class_hash)
                .await?
            {
                ContractClass::Legacy(legacy) => {
                    trace!(target: LOG_TARGET, version = "cairo 0", %class_hash, "Set contract class.");

                    let (hash, class) = rpc_converter::legacy_rpc_to_compiled_class(&legacy)?;
                    state_updates_with_classes.declared_compiled_classes.insert(hash, class);
                }
                ContractClass::Sierra(s) => {
                    trace!(target: LOG_TARGET, version = "cairo 1", %class_hash, "Set contract class.");

                    state_updates_with_classes
                        .declared_sierra_classes
                        .insert(*class_hash, s.clone());
                }
            }
        }

        Ok((state_updates_with_classes, serialized_state_update))
    }

    async fn fetch_transactions_executions(
        &self,
        block_number: BlockNumber,
    ) -> ProviderResult<Vec<TxExecutionInfo>> {
        trace!(target: LOG_TARGET, block_number = %block_number, "Fetching transactions executions.");

        let client = HttpClientBuilder::default().build(&self.rpc_url).unwrap();

        let block_id = BlockIdOrTag::Number(block_number);
        let traces = client.transaction_executions_by_block(block_id).await.unwrap();

        Ok(traces)
    }
}
