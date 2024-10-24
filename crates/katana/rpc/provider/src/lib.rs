use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::chain::ChainId;
use katana_primitives::contract::StorageKey;
use katana_primitives::transaction::TxHash;
use katana_primitives::{ContractAddress, Felt};
use katana_provider::traits::block::{BlockHashProvider, BlockProvider};
use katana_provider::traits::state::StateRootProvider;
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_rpc_types::block::{BlockWithReceipts, BlockWithTxHashes, BlockWithTxs};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::event::{EventFilterWithPage, EventsPage};
use katana_rpc_types::receipt::{ReceiptBlock, TxReceiptWithBlockInfo};
use katana_rpc_types::state_update::{StateDiff, StateUpdate};
use katana_rpc_types::transaction::Tx;

pub type StarknetApiResult<T> = Result<T, StarknetApiError>;

pub trait StarknetApiProvider {
    fn block_state_update(
        &self,
        block: BlockHashOrNumber,
    ) -> StarknetApiResult<Option<StateUpdate>> {
        todo!()
    }

    fn storage(
        &self,
        address: ContractAddress,
        key: StorageKey,
        block: BlockHashOrNumber,
    ) -> StarknetApiResult<Felt> {
        todo!()
    }

    fn nonce(&self, address: ContractAddress, block: BlockHashOrNumber) -> StarknetApiResult<Felt> {
        todo!()
    }

    fn chain_id(&self) -> StarknetApiResult<ChainId> {
        todo!()
    }

    fn events(&self, filter: EventFilterWithPage) -> StarknetApiResult<EventsPage> {
        todo!()
    }

    fn block_with_txs(&self, block: BlockHashOrNumber) -> StarknetApiResult<Option<BlockWithTxs>>;

    fn block_with_txs_hashes(
        &self,
        block: BlockHashOrNumber,
    ) -> StarknetApiResult<Option<BlockWithTxHashes>>;

    fn block_with_receipts(
        &self,
        block: BlockHashOrNumber,
    ) -> StarknetApiResult<Option<BlockWithReceipts>>;

    fn transaction(&self, hash: TxHash) -> StarknetApiResult<Option<Tx>> {
        todo!()
    }

    fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockHashOrNumber,
        index: u64,
    ) -> StarknetApiResult<Option<Tx>> {
        todo!()
    }

    fn receipt(&self, hash: TxHash) -> StarknetApiResult<Option<TxReceiptWithBlockInfo>> {
        todo!()
    }
}

impl<P> StarknetApiProvider for P
where
    P: BlockProvider
        + BlockHashProvider
        + ReceiptProvider
        + TransactionProvider
        + TransactionStatusProvider
        + StateRootProvider
        + StateUpdateProvider,
{
    fn block_with_txs(&self, id: BlockHashOrNumber) -> StarknetApiResult<Option<BlockWithTxs>> {
        let Some(hash) = self.block_hash_by_id(id)? else {
            return Ok(None);
        };

        let block = self.block(id)?.expect("should exist if hash exists");
        let status = self.block_status(id)?.expect("should exist if block exists");

        Ok(Some(BlockWithTxs::new(hash, block, status)))
    }

    fn block_with_receipts(
        &self,
        id: BlockHashOrNumber,
    ) -> StarknetApiResult<Option<BlockWithReceipts>> {
        let Some(hash) = self.block_hash_by_id(id)? else {
            return Ok(None);
        };

        let block = self.block(id)?.expect("should exist if hash exists");
        let status = self.block_status(id)?.expect("should exist if block exists");
        let receipts = self.receipts_by_block(id)?.expect("should exist if block exists");

        Ok(Some(BlockWithReceipts::new(hash, block, status, receipts)))
    }

    fn block_with_txs_hashes(
        &self,
        id: BlockHashOrNumber,
    ) -> StarknetApiResult<Option<BlockWithTxHashes>> {
        let Some(hash) = self.block_hash_by_id(id)? else {
            return Ok(None);
        };

        let block = self.block_with_tx_hashes(id)?.expect("should exist if block exists");
        let status = self.block_status(id)?.expect("should exist if block exists");

        Ok(Some(BlockWithTxHashes::new(hash, block, status)))
    }

    fn receipt(&self, hash: TxHash) -> StarknetApiResult<Option<TxReceiptWithBlockInfo>> {
        let Some(receipt) = self.receipt_by_hash(hash)? else { return Ok(None) };

        let (num, hash) = self.transaction_block_num_and_hash(hash)?.expect("must exist");
        let status = self.transaction_status(hash)?.expect("must exist");
        let block = ReceiptBlock::Block { block_hash: hash, block_number: num };

        Ok(Some(TxReceiptWithBlockInfo::new(block, hash, status, receipt)))
    }

    fn block_state_update(
        &self,
        block: BlockHashOrNumber,
    ) -> StarknetApiResult<Option<StateUpdate>> {
        let Some(hash) = self.block_hash_by_id(block)? else {
            return Ok(None);
        };

        let new_root = self.state_root(block)?.expect("should exist if block exists");
        let block_num = self.block_number_by_hash(hash)?.expect("should exist if block exists");
        let old_root = match block_num {
            0 => Felt::ZERO,
            _ => self.state_root((block_num - 1).into())?.expect("should exist if not genesis"),
        };

        let state_diff = self.state_update(block)?.expect("should exist if block exists");
        let state_diff: StateDiff = state_diff.into();

        Ok(Some(
            starknet::core::types::StateUpdate {
                new_root,
                old_root,
                block_hash: hash,
                state_diff: state_diff.0,
            }
            .into(),
        ))
    }
}
