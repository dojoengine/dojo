use katana_primitives::block::{BlockHashOrNumber, BlockNumber, FinalityStatus};
use katana_primitives::transaction::TxHash;
use katana_primitives::{felt, Felt};
use katana_provider::traits::block::{BlockHashProvider, BlockProvider};
use katana_provider::traits::state::{StateFactoryProvider, StateRootProvider};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_rpc_types::block::{
    BlockHashAndNumber, BlockWithReceipts, BlockWithTxHashes, BlockWithTxs,
};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::receipt::{ReceiptBlock, TxReceiptWithBlockInfo};
use katana_rpc_types::state_update::{StateDiff, StateUpdate};
use katana_rpc_types::transaction::Tx;
use starknet::core::types::{TransactionExecutionStatus, TransactionStatus};

pub type StarknetApiResult<T> = Result<T, StarknetApiError>;

pub trait StarknetProvider {
    // fn events(&self, filter: EventFilterWithPage) -> StarknetApiResult<EventsPage> {
    //     todo!()
    // }

    fn block_number(&self) -> StarknetApiResult<BlockNumber>;

    fn block_hash_and_number(&self) -> StarknetApiResult<BlockHashAndNumber>;

    fn block_with_txs(&self, block: BlockHashOrNumber) -> StarknetApiResult<BlockWithTxs>;

    fn block_with_txs_hashes(
        &self,
        block: BlockHashOrNumber,
    ) -> StarknetApiResult<BlockWithTxHashes>;

    fn block_with_receipts(&self, block: BlockHashOrNumber)
    -> StarknetApiResult<BlockWithReceipts>;

    fn block_state_update(&self, block: BlockHashOrNumber) -> StarknetApiResult<StateUpdate>;

    fn block_transaction_count(&self, block: BlockHashOrNumber) -> StarknetApiResult<u64>;

    fn transaction(&self, hash: TxHash) -> StarknetApiResult<Tx>;

    fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockHashOrNumber,
        index: u64,
    ) -> StarknetApiResult<Tx>;

    fn transaction_status(&self, hash: TxHash) -> StarknetApiResult<TransactionStatus>;

    fn receipt(&self, hash: TxHash) -> StarknetApiResult<TxReceiptWithBlockInfo>;
}

impl<P> StarknetProvider for P
where
    P: BlockProvider
        + StateFactoryProvider
        + BlockHashProvider
        + ReceiptProvider
        + TransactionProvider
        + TransactionStatusProvider
        + StateRootProvider
        + StateUpdateProvider,
{
    fn block_number(&self) -> StarknetApiResult<BlockNumber> {
        Ok(self.latest_number()?)
    }

    fn block_hash_and_number(&self) -> StarknetApiResult<BlockHashAndNumber> {
        let hash = self.latest_hash()?;
        let number = self.latest_number()?;
        Ok(BlockHashAndNumber::new(hash, number))
    }

    fn block_with_txs(&self, id: BlockHashOrNumber) -> StarknetApiResult<BlockWithTxs> {
        let hash = self.block_hash_by_id(id)?.ok_or(StarknetApiError::BlockNotFound)?;
        let block = self.block(id)?.expect("should exist if hash exists");
        let status = self.block_status(id)?.expect("should exist if block exists");
        Ok(BlockWithTxs::new(hash, block, status))
    }

    fn block_with_receipts(&self, id: BlockHashOrNumber) -> StarknetApiResult<BlockWithReceipts> {
        let hash = self.block_hash_by_id(id)?.ok_or(StarknetApiError::BlockNotFound)?;
        let block = self.block(id)?.expect("should exist if hash exists");

        let status = self.block_status(id)?.expect("should exist if block exists");
        let receipts = self.receipts_by_block(id)?.expect("should exist if block exists");

        Ok(BlockWithReceipts::new(hash, block, status, receipts))
    }

    fn block_with_txs_hashes(&self, id: BlockHashOrNumber) -> StarknetApiResult<BlockWithTxHashes> {
        let hash = self.block_hash_by_id(id)?.ok_or(StarknetApiError::BlockNotFound)?;
        let block = self.block_with_tx_hashes(id)?.expect("should exist if block exists");
        let status = self.block_status(id)?.expect("should exist if block exists");
        Ok(BlockWithTxHashes::new(hash, block, status))
    }

    fn block_transaction_count(&self, id: BlockHashOrNumber) -> StarknetApiResult<u64> {
        let count = TransactionProvider::transaction_count_by_block(&self, id)?;
        Ok(count.ok_or(StarknetApiError::BlockNotFound)?)
    }

    fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockHashOrNumber,
        index: u64,
    ) -> StarknetApiResult<Tx> {
        let tx = self
            .transaction_by_block_and_idx(block_id, index)?
            .ok_or(StarknetApiError::TxnHashNotFound)?;
        Ok(tx.into())
    }

    fn transaction(&self, hash: TxHash) -> StarknetApiResult<Tx> {
        let tx = self.transaction_by_hash(hash)?.ok_or(StarknetApiError::TxnHashNotFound)?;
        Ok(tx.into())
    }

    fn transaction_status(&self, hash: TxHash) -> StarknetApiResult<TransactionStatus> {
        let status = self.transaction_status(hash)?.ok_or(StarknetApiError::TxnHashNotFound)?;
        let receipt = self.receipt_by_hash(hash)?.expect("must exist");

        let exec_status = if receipt.is_reverted() {
            TransactionExecutionStatus::Reverted
        } else {
            TransactionExecutionStatus::Succeeded
        };

        let status = match status {
            FinalityStatus::AcceptedOnL1 => TransactionStatus::AcceptedOnL1(exec_status),
            FinalityStatus::AcceptedOnL2 => TransactionStatus::AcceptedOnL2(exec_status),
        };

        Ok(status)
    }

    fn receipt(&self, hash: TxHash) -> StarknetApiResult<TxReceiptWithBlockInfo> {
        let receipt = self.receipt_by_hash(hash)?.ok_or(StarknetApiError::TxnHashNotFound)?;

        let (num, hash) = self.transaction_block_num_and_hash(hash)?.expect("must exist");
        let status = self.transaction_status(hash)?.expect("must exist");
        let block = ReceiptBlock::Block { block_hash: hash, block_number: num };

        Ok(TxReceiptWithBlockInfo::new(block, hash, status, receipt))
    }

    fn block_state_update(&self, block: BlockHashOrNumber) -> StarknetApiResult<StateUpdate> {
        let hash = self.block_hash_by_id(block)?.ok_or(StarknetApiError::BlockNotFound)?;

        let new_root = self.state_root(block)?.expect("should exist if block exists");
        let block_num = self.block_number_by_hash(hash)?.expect("should exist if block exists");
        let old_root = match block_num {
            0 => Felt::ZERO,
            _ => self.state_root((block_num - 1).into())?.expect("should exist if not genesis"),
        };

        let state_diff = self.state_update(block)?.expect("should exist if block exists");
        let state_diff: StateDiff = state_diff.into();

        Ok(starknet::core::types::StateUpdate {
            new_root,
            old_root,
            block_hash: hash,
            state_diff: state_diff.0,
        }
        .into())
    }
}
