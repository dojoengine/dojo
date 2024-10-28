use katana_primitives::block::{BlockIdOrTag, BlockNumber};
use katana_primitives::transaction::TxHash;
use katana_rpc_types::block::{
    MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
};
use katana_rpc_types::receipt::TxReceiptWithBlockInfo;
use katana_rpc_types::state_update::MaybePendingStateUpdate;
use katana_rpc_types::transaction::Tx;
use starknet::core::types::TransactionStatus;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use url::Url;

use super::StarknetApiResult;

#[derive(Debug)]
pub struct ForkedClient<P: Provider = JsonRpcClient<HttpTransport>> {
    #[allow(unused)]
    block: BlockNumber,
    provider: P,
}

impl<P: Provider> ForkedClient<P> {
    /// Creates a new forked client from the given [`Provider`] and block number.
    pub fn new(provider: P, block: BlockNumber) -> Self {
        Self { provider, block }
    }

    /// Returns the block number of the forked client.
    pub fn block(&self) -> &BlockNumber {
        &self.block
    }
}

impl ForkedClient {
    /// Creates a new forked client from the given HTTP URL and block number.
    pub fn new_http(url: Url, block: BlockNumber) -> Self {
        Self { provider: JsonRpcClient::new(HttpTransport::new(url)), block }
    }
}

impl<P: Provider> ForkedClient<P> {
    pub async fn get_transaction_by_hash(&self, hash: TxHash) -> StarknetApiResult<Tx> {
        let tx = self.provider.get_transaction_by_hash(hash).await?;
        Ok(tx.into())
    }

    pub async fn get_transaction_receipt(
        &self,
        hash: TxHash,
    ) -> StarknetApiResult<TxReceiptWithBlockInfo> {
        let receipt = self.provider.get_transaction_receipt(hash).await?;
        Ok(receipt.into())
    }

    pub async fn get_transaction_status(
        &self,
        hash: TxHash,
    ) -> StarknetApiResult<TransactionStatus> {
        let status = self.provider.get_transaction_status(hash).await?;
        Ok(status)
    }

    pub async fn get_transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> StarknetApiResult<Tx> {
        let tx = self.provider.get_transaction_by_block_id_and_index(block_id, index).await?;
        Ok(tx.into())
    }

    pub async fn get_block_with_txs(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingBlockWithTxs> {
        let block = self.provider.get_block_with_txs(block_id).await?;
        Ok(block.into())
    }

    pub async fn get_block_with_receipts(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingBlockWithReceipts> {
        let block = self.provider.get_block_with_receipts(block_id).await?;
        Ok(block.into())
    }

    pub async fn get_block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingBlockWithTxHashes> {
        let block = self.provider.get_block_with_tx_hashes(block_id).await?;
        Ok(block.into())
    }

    pub async fn get_block_transaction_count(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<u64> {
        let status = self.provider.get_block_transaction_count(block_id).await?;
        Ok(status)
    }

    pub async fn get_state_update(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingStateUpdate> {
        let state_update = self.provider.get_state_update(block_id).await?;
        Ok(state_update.into())
    }
}
