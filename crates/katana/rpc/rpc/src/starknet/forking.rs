use katana_primitives::block::{BlockIdOrTag, BlockNumber};
use katana_primitives::transaction::TxHash;
use katana_rpc_types::block::{
    MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::receipt::TxReceiptWithBlockInfo;
use katana_rpc_types::state_update::MaybePendingStateUpdate;
use katana_rpc_types::transaction::Tx;
use starknet::core::types::TransactionStatus;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error originating from the underlying [`Provider`] implementation.
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Block out of range")]
    BlockOutOfRange,
}

#[derive(Debug)]
pub struct ForkedClient<P: Provider = JsonRpcClient<HttpTransport>> {
    /// The block number where the node is forked from.
    block: BlockNumber,
    /// The Starknet Json RPC provider client for doing the request to the forked network.
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
    pub async fn get_transaction_by_hash(&self, hash: TxHash) -> Result<Tx, Error> {
        let tx = self.provider.get_transaction_by_hash(hash).await?;
        Ok(tx.into())
    }

    pub async fn get_transaction_receipt(
        &self,
        hash: TxHash,
    ) -> Result<TxReceiptWithBlockInfo, Error> {
        let receipt = self.provider.get_transaction_receipt(hash).await?;

        if let starknet::core::types::ReceiptBlock::Block { block_number, .. } = receipt.block {
            if block_number > self.block {
                return Err(Error::BlockOutOfRange);
            }
        }

        Ok(receipt.into())
    }

    pub async fn get_transaction_status(&self, hash: TxHash) -> Result<TransactionStatus, Error> {
        let (receipt, status) = tokio::join!(
            self.get_transaction_receipt(hash),
            self.provider.get_transaction_status(hash)
        );

        // We get the receipt first to check if the block number is within the forked range.
        let _ = receipt?;

        Ok(status?)
    }

    pub async fn get_transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        idx: u64,
    ) -> Result<Tx, Error> {
        match block_id {
            BlockIdOrTag::Number(num) => {
                if num > self.block {
                    return Err(Error::BlockOutOfRange);
                }

                let tx = self.provider.get_transaction_by_block_id_and_index(block_id, idx).await?;
                Ok(tx.into())
            }

            BlockIdOrTag::Hash(hash) => {
                let (block, tx) = tokio::join!(
                    self.provider.get_block_with_tx_hashes(BlockIdOrTag::Hash(hash)),
                    self.provider.get_transaction_by_block_id_and_index(block_id, idx)
                );

                let number = match block? {
                    starknet::core::types::MaybePendingBlockWithTxHashes::Block(block) => {
                        block.block_number
                    }
                    starknet::core::types::MaybePendingBlockWithTxHashes::PendingBlock(_) => {
                        panic!("shouldn't be possible to be pending")
                    }
                };

                if number > self.block {
                    return Err(Error::BlockOutOfRange);
                }

                Ok(tx?.into())
            }

            BlockIdOrTag::Tag(_) => {
                panic!("shouldn't be possible to be tag")
            }
        }
    }

    pub async fn get_block_with_txs(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingBlockWithTxs, Error> {
        let block = self.provider.get_block_with_txs(block_id).await?;

        match block {
            starknet::core::types::MaybePendingBlockWithTxs::Block(ref b) => {
                if b.block_number > self.block {
                    Err(Error::BlockOutOfRange)
                } else {
                    Ok(block.into())
                }
            }

            starknet::core::types::MaybePendingBlockWithTxs::PendingBlock(_) => {
                panic!("shouldn't be possible to be pending")
            }
        }
    }

    pub async fn get_block_with_receipts(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingBlockWithReceipts, Error> {
        let block = self.provider.get_block_with_receipts(block_id).await?;

        match block {
            starknet::core::types::MaybePendingBlockWithReceipts::Block(ref b) => {
                if b.block_number > self.block {
                    return Err(Error::BlockOutOfRange);
                }
            }
            starknet::core::types::MaybePendingBlockWithReceipts::PendingBlock(_) => {
                panic!("shouldn't be possible to be pending")
            }
        }

        Ok(block.into())
    }

    pub async fn get_block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingBlockWithTxHashes, Error> {
        let block = self.provider.get_block_with_tx_hashes(block_id).await?;

        match block {
            starknet::core::types::MaybePendingBlockWithTxHashes::Block(ref b) => {
                if b.block_number > self.block {
                    return Err(Error::BlockOutOfRange);
                }
            }
            starknet::core::types::MaybePendingBlockWithTxHashes::PendingBlock(_) => {
                panic!("shouldn't be possible to be pending")
            }
        }

        Ok(block.into())
    }

    pub async fn get_block_transaction_count(&self, block_id: BlockIdOrTag) -> Result<u64, Error> {
        match block_id {
            BlockIdOrTag::Number(num) if num > self.block => {
                return Err(Error::BlockOutOfRange);
            }
            BlockIdOrTag::Hash(hash) => {
                let block =
                    self.provider.get_block_with_tx_hashes(BlockIdOrTag::Hash(hash)).await?;
                if let starknet::core::types::MaybePendingBlockWithTxHashes::Block(b) = block {
                    if b.block_number > self.block {
                        return Err(Error::BlockOutOfRange);
                    }
                }
            }
            BlockIdOrTag::Tag(_) => {
                panic!("shouldn't be possible to be tag")
            }
            _ => {}
        }

        let status = self.provider.get_block_transaction_count(block_id).await?;
        Ok(status)
    }

    pub async fn get_state_update(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingStateUpdate, Error> {
        match block_id {
            BlockIdOrTag::Number(num) if num > self.block => {
                return Err(Error::BlockOutOfRange);
            }
            BlockIdOrTag::Hash(hash) => {
                let block =
                    self.provider.get_block_with_tx_hashes(BlockIdOrTag::Hash(hash)).await?;
                if let starknet::core::types::MaybePendingBlockWithTxHashes::Block(b) = block {
                    if b.block_number > self.block {
                        return Err(Error::BlockOutOfRange);
                    }
                }
            }
            BlockIdOrTag::Tag(_) => {
                panic!("shouldn't be possible to be tag")
            }
            _ => {}
        }

        let state_update = self.provider.get_state_update(block_id).await?;
        Ok(state_update.into())
    }
}

impl From<Error> for StarknetApiError {
    fn from(value: Error) -> Self {
        match value {
            Error::Provider(provider_error) => provider_error.into(),
            Error::BlockOutOfRange => StarknetApiError::BlockNotFound,
        }
    }
}
