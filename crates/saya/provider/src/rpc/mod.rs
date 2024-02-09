//! Provider to fetch Katana data from RPC.
//!
//! The transport here is fixed to JSON RPC.
use std::sync::Arc;

use anyhow::anyhow;
use katana_primitives::block::{
    BlockIdOrTag, BlockNumber, GasPrices, Header, SealedBlock, SealedHeader,
};
use katana_primitives::chain::ChainId;
use katana_primitives::conversion::rpc as rpc_converter;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::TxWithHash;
use katana_primitives::version::Version;
use starknet::core::types::{
    BlockId, ContractClass, DeclaredClassItem, MaybePendingBlockWithTxs, MaybePendingStateUpdate,
    StateUpdate, Transaction,
};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider as StarknetProvider};
use tracing::{error, trace};
use url::Url;

use crate::provider::Provider;
use crate::rpc::{state as state_converter, transaction as tx_converter};
use crate::{ProviderResult, LOG_TARGET};

mod state;
mod transaction;

/// A JSON RPC provider.
pub struct JsonRpcProvider {
    provider: Arc<JsonRpcClient<HttpTransport>>,
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
        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(rpc_url)));

        let chain_id = provider.chain_id().await?;

        Ok(Self { provider, chain_id: chain_id.into() })
    }

    /// Returns the internal [`ChainId`].
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }
}

#[async_trait::async_trait]
impl Provider for JsonRpcProvider {
    async fn block_number(&self) -> ProviderResult<BlockNumber> {
        Ok(self.provider.block_number().await?)
    }

    async fn fetch_block(&self, block_number: BlockNumber) -> ProviderResult<SealedBlock> {
        let block =
            match self.provider.get_block_with_txs(BlockIdOrTag::Number(block_number)).await? {
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
                hash: block.block_hash.into(),
                header: Header {
                    parent_hash: block.parent_hash.into(),
                    number: block.block_number,
                    gas_prices: GasPrices::new(
                        block.l1_gas_price.price_in_wei,
                        block.l1_gas_price.price_in_strk.unwrap_or_default(),
                    ),
                    timestamp: block.timestamp,
                    state_root: block.new_root,
                    sequencer_address: block.sequencer_address.into(),
                    version: Version::parse(&block.starknet_version)?,
                },
            },
            body: txs,
        })
    }

    async fn fetch_state_updates(
        &self,
        block_number: BlockNumber,
    ) -> ProviderResult<StateUpdatesWithDeclaredClasses> {
        let rpc_state_update =
            match self.provider.get_state_update(BlockIdOrTag::Number(block_number)).await? {
                MaybePendingStateUpdate::Update(su) => su,
                MaybePendingStateUpdate::PendingUpdate(_) => {
                    return Err(anyhow!("PendingUpdate should not be fetched").into());
                }
            };

        let state_updates = state_converter::state_updates_from_rpc(&rpc_state_update)?;

        let mut state_updates_with_classes =
            StateUpdatesWithDeclaredClasses { state_updates, ..Default::default() };

        for (class_hash, _) in &state_updates_with_classes.state_updates.declared_classes {
            match self.provider.get_class(BlockIdOrTag::Number(block_number), class_hash).await? {
                ContractClass::Legacy(legacy) => {
                    trace!(target: LOG_TARGET, version = "cairo 0", %class_hash, "set contract class");

                    let (hash, class) = rpc_converter::legacy_rpc_to_inner_compiled_class(&legacy)?;
                    state_updates_with_classes.declared_compiled_classes.insert(hash, class);
                }
                ContractClass::Sierra(s) => {
                    trace!(target: LOG_TARGET, version = "cairo 1", %class_hash, "set contract class");

                    state_updates_with_classes
                        .declared_sierra_classes
                        .insert(*class_hash, s.clone());
                }
            }
        }

        Ok(state_updates_with_classes)
    }
}
