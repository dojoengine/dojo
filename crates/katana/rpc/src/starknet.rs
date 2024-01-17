use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error};
use katana_core::backend::contract::StarknetContract;
use katana_core::sequencer::KatanaSequencer;
use katana_executor::blockifier::utils::EntryPointCall;
use katana_primitives::block::{
    BlockHashOrNumber, BlockIdOrTag, FinalityStatus, GasPrices, PartialHeader,
};
use katana_primitives::conversion::rpc::legacy_inner_to_rpc_class;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxHash};
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::FieldElement;
use katana_provider::traits::block::{BlockHashProvider, BlockIdReader, BlockNumberProvider};
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_rpc_types::block::{
    BlockHashAndNumber, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    PendingBlockWithTxHashes, PendingBlockWithTxs,
};
use katana_rpc_types::event::{EventFilterWithPage, EventsPage};
use katana_rpc_types::message::MsgFromL1;
use katana_rpc_types::receipt::{MaybePendingTxReceipt, PendingTxReceipt};
use katana_rpc_types::state_update::StateUpdate;
use katana_rpc_types::transaction::{
    BroadcastedDeclareTx, BroadcastedDeployAccountTx, BroadcastedInvokeTx, BroadcastedTx,
    DeclareTxResult, DeployAccountTxResult, InvokeTxResult, Tx,
};
use katana_rpc_types::{ContractClass, FeeEstimate, FeltAsHex, FunctionCall};
use katana_rpc_types_builder::ReceiptBuilder;
use starknet::core::types::{BlockTag, TransactionExecutionStatus, TransactionStatus};

use crate::api::starknet::{StarknetApiError, StarknetApiServer};

pub struct StarknetApi {
    sequencer: Arc<KatanaSequencer>,
}

impl StarknetApi {
    pub fn new(sequencer: Arc<KatanaSequencer>) -> Self {
        Self { sequencer }
    }
}
#[async_trait]
impl StarknetApiServer for StarknetApi {
    async fn chain_id(&self) -> Result<FeltAsHex, Error> {
        Ok(FieldElement::from(self.sequencer.chain_id()).into())
    }

    async fn nonce(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> Result<FeltAsHex, Error> {
        let nonce = self
            .sequencer
            .nonce_at(block_id, contract_address.into())
            .await
            .map_err(StarknetApiError::from)?
            .ok_or(StarknetApiError::ContractNotFound)?;

        Ok(nonce.into())
    }

    async fn block_number(&self) -> Result<u64, Error> {
        Ok(self.sequencer.block_number())
    }

    async fn transaction_by_hash(&self, transaction_hash: FieldElement) -> Result<Tx, Error> {
        let tx = self
            .sequencer
            .transaction(&transaction_hash)
            .map_err(StarknetApiError::from)?
            .ok_or(StarknetApiError::TxnHashNotFound)?;
        Ok(tx.into())
    }

    async fn block_transaction_count(&self, block_id: BlockIdOrTag) -> Result<u64, Error> {
        let count = self
            .sequencer
            .block_tx_count(block_id)
            .map_err(StarknetApiError::from)?
            .ok_or(StarknetApiError::BlockNotFound)?;
        Ok(count)
    }

    async fn class_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> Result<ContractClass, Error> {
        let class_hash = self
            .sequencer
            .class_hash_at(block_id, contract_address.into())
            .map_err(StarknetApiError::from)?
            .ok_or(StarknetApiError::ContractNotFound)?;

        self.class(block_id, class_hash).await
    }

    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error> {
        let hash_and_num_pair =
            self.sequencer.block_hash_and_number().map_err(StarknetApiError::from)?;
        Ok(hash_and_num_pair.into())
    }

    async fn block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingBlockWithTxHashes, Error> {
        let provider = self.sequencer.backend.blockchain.provider();

        if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
            if let Some(pending_state) = self.sequencer.pending_state() {
                let block_env = pending_state.block_envs.read().0.clone();
                let latest_hash =
                    BlockHashProvider::latest_hash(provider).map_err(StarknetApiError::from)?;

                let gas_prices = GasPrices {
                    eth: block_env.l1_gas_prices.eth,
                    strk: block_env.l1_gas_prices.strk,
                };

                let header = PartialHeader {
                    gas_prices,
                    parent_hash: latest_hash,
                    version: CURRENT_STARKNET_VERSION,
                    timestamp: block_env.timestamp,
                    sequencer_address: block_env.sequencer_address,
                };

                let transactions = pending_state
                    .executed_txs
                    .read()
                    .iter()
                    .map(|(tx, _)| tx.hash)
                    .collect::<Vec<_>>();

                return Ok(MaybePendingBlockWithTxHashes::Pending(PendingBlockWithTxHashes::new(
                    header,
                    transactions,
                )));
            }
        }

        let block_num = BlockIdReader::convert_block_id(provider, block_id)
            .map_err(StarknetApiError::from)?
            .map(BlockHashOrNumber::Num)
            .ok_or(StarknetApiError::BlockNotFound)?;

        katana_rpc_types_builder::BlockBuilder::new(block_num, provider)
            .build_with_tx_hash()
            .map_err(StarknetApiError::from)?
            .map(MaybePendingBlockWithTxHashes::Block)
            .ok_or(Error::from(StarknetApiError::BlockNotFound))
    }

    async fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> Result<Tx, Error> {
        // TEMP: have to handle pending tag independently for now
        let tx = if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
            let Some(pending_state) = self.sequencer.pending_state() else {
                return Err(StarknetApiError::BlockNotFound.into());
            };

            let pending_txs = pending_state.executed_txs.read();
            pending_txs.iter().nth(index as usize).map(|(tx, _)| tx.clone())
        } else {
            let provider = &self.sequencer.backend.blockchain.provider();

            let block_num = BlockIdReader::convert_block_id(provider, block_id)
                .map_err(StarknetApiError::from)?
                .map(BlockHashOrNumber::Num)
                .ok_or(StarknetApiError::BlockNotFound)?;

            TransactionProvider::transaction_by_block_and_idx(provider, block_num, index)
                .map_err(StarknetApiError::from)?
        };

        Ok(tx.ok_or(StarknetApiError::InvalidTxnIndex)?.into())
    }

    async fn block_with_txs(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<MaybePendingBlockWithTxs, Error> {
        let provider = self.sequencer.backend.blockchain.provider();

        if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
            if let Some(pending_state) = self.sequencer.pending_state() {
                let block_env = pending_state.block_envs.read().0.clone();
                let latest_hash =
                    BlockHashProvider::latest_hash(provider).map_err(StarknetApiError::from)?;

                let gas_prices = GasPrices {
                    eth: block_env.l1_gas_prices.eth,
                    strk: block_env.l1_gas_prices.strk,
                };

                let header = PartialHeader {
                    gas_prices,
                    parent_hash: latest_hash,
                    version: CURRENT_STARKNET_VERSION,
                    timestamp: block_env.timestamp,
                    sequencer_address: block_env.sequencer_address,
                };

                let transactions = pending_state
                    .executed_txs
                    .read()
                    .iter()
                    .map(|(tx, _)| tx.clone())
                    .collect::<Vec<_>>();

                return Ok(MaybePendingBlockWithTxs::Pending(PendingBlockWithTxs::new(
                    header,
                    transactions,
                )));
            }
        }

        let block_num = BlockIdReader::convert_block_id(provider, block_id)
            .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?
            .map(BlockHashOrNumber::Num)
            .ok_or(StarknetApiError::BlockNotFound)?;

        katana_rpc_types_builder::BlockBuilder::new(block_num, provider)
            .build()
            .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?
            .map(MaybePendingBlockWithTxs::Block)
            .ok_or(Error::from(StarknetApiError::BlockNotFound))
    }

    async fn state_update(&self, block_id: BlockIdOrTag) -> Result<StateUpdate, Error> {
        let provider = self.sequencer.backend.blockchain.provider();

        let block_id = match block_id {
            BlockIdOrTag::Number(num) => BlockHashOrNumber::Num(num),
            BlockIdOrTag::Hash(hash) => BlockHashOrNumber::Hash(hash),

            BlockIdOrTag::Tag(BlockTag::Latest) => BlockNumberProvider::latest_number(provider)
                .map(BlockHashOrNumber::Num)
                .map_err(|_| StarknetApiError::BlockNotFound)?,

            BlockIdOrTag::Tag(BlockTag::Pending) => {
                return Err(StarknetApiError::BlockNotFound.into());
            }
        };

        katana_rpc_types_builder::StateUpdateBuilder::new(block_id, provider)
            .build()
            .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?
            .ok_or(Error::from(StarknetApiError::BlockNotFound))
    }

    async fn transaction_receipt(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<MaybePendingTxReceipt, Error> {
        let provider = self.sequencer.backend.blockchain.provider();
        let receipt = ReceiptBuilder::new(transaction_hash, provider)
            .build()
            .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?;

        match receipt {
            Some(receipt) => Ok(MaybePendingTxReceipt::Receipt(receipt)),

            None => {
                let pending_receipt = self.sequencer.pending_state().and_then(|s| {
                    s.executed_txs
                        .read()
                        .iter()
                        .find(|(tx, _)| tx.hash == transaction_hash)
                        .map(|(_, rct)| rct.receipt.clone())
                });

                let Some(pending_receipt) = pending_receipt else {
                    return Err(StarknetApiError::TxnHashNotFound.into());
                };

                Ok(MaybePendingTxReceipt::Pending(PendingTxReceipt::new(
                    transaction_hash,
                    pending_receipt,
                )))
            }
        }
    }

    async fn class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> Result<FeltAsHex, Error> {
        let hash = self
            .sequencer
            .class_hash_at(block_id, contract_address.into())
            .map_err(StarknetApiError::from)?
            .ok_or(StarknetApiError::ContractNotFound)?;

        Ok(hash.into())
    }

    async fn class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: FieldElement,
    ) -> Result<ContractClass, Error> {
        let class = self.sequencer.class(block_id, class_hash).map_err(StarknetApiError::from)?;
        let Some(class) = class else { return Err(StarknetApiError::ClassHashNotFound.into()) };

        match class {
            StarknetContract::Legacy(c) => {
                let contract = legacy_inner_to_rpc_class(c)
                    .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?;
                Ok(contract)
            }
            StarknetContract::Sierra(c) => Ok(ContractClass::Sierra(c)),
        }
    }

    async fn events(&self, filter: EventFilterWithPage) -> Result<EventsPage, Error> {
        let from_block = filter.event_filter.from_block.unwrap_or(BlockIdOrTag::Number(0));
        let to_block = filter.event_filter.to_block.unwrap_or(BlockIdOrTag::Tag(BlockTag::Latest));

        let keys = filter.event_filter.keys;
        let keys = keys.filter(|keys| !(keys.len() == 1 && keys.is_empty()));

        let events = self
            .sequencer
            .events(
                from_block,
                to_block,
                filter.event_filter.address.map(|f| f.into()),
                keys,
                filter.result_page_request.continuation_token,
                filter.result_page_request.chunk_size,
            )
            .await
            .map_err(StarknetApiError::from)?;

        Ok(events)
    }

    async fn call(
        &self,
        request: FunctionCall,
        block_id: BlockIdOrTag,
    ) -> Result<Vec<FeltAsHex>, Error> {
        let request = EntryPointCall {
            calldata: request.calldata,
            contract_address: request.contract_address.into(),
            entry_point_selector: request.entry_point_selector,
        };

        let res = self.sequencer.call(request, block_id).map_err(StarknetApiError::from)?;

        Ok(res.into_iter().map(|v| v.into()).collect())
    }

    async fn storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        block_id: BlockIdOrTag,
    ) -> Result<FeltAsHex, Error> {
        let value = self
            .sequencer
            .storage_at(contract_address.into(), key, block_id)
            .map_err(StarknetApiError::from)?;

        Ok(value.into())
    }

    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTx,
    ) -> Result<DeployAccountTxResult, Error> {
        if deploy_account_transaction.is_query {
            return Err(StarknetApiError::UnsupportedTransactionVersion.into());
        }

        let chain_id = self.sequencer.chain_id();

        let tx = deploy_account_transaction.into_tx_with_chain_id(chain_id);
        let contract_address = tx.contract_address;

        let tx = ExecutableTxWithHash::new(ExecutableTx::DeployAccount(tx));
        let tx_hash = tx.hash;

        self.sequencer.add_transaction_to_pool(tx);

        Ok((tx_hash, contract_address).into())
    }

    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTx>,
        block_id: BlockIdOrTag,
    ) -> Result<Vec<FeeEstimate>, Error> {
        let chain_id = self.sequencer.chain_id();

        let transactions = request
            .into_iter()
            .map(|tx| {
                let tx = match tx {
                    BroadcastedTx::Invoke(tx) => {
                        let tx = tx.into_tx_with_chain_id(chain_id);
                        ExecutableTxWithHash::new_query(ExecutableTx::Invoke(tx))
                    }

                    BroadcastedTx::DeployAccount(tx) => {
                        let tx = tx.into_tx_with_chain_id(chain_id);
                        ExecutableTxWithHash::new_query(ExecutableTx::DeployAccount(tx))
                    }

                    BroadcastedTx::Declare(tx) => {
                        let tx = tx
                            .try_into_tx_with_chain_id(chain_id)
                            .map_err(|_| StarknetApiError::InvalidContractClass)?;
                        ExecutableTxWithHash::new_query(ExecutableTx::Declare(tx))
                    }
                };

                Result::<ExecutableTxWithHash, StarknetApiError>::Ok(tx)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let res =
            self.sequencer.estimate_fee(transactions, block_id).map_err(StarknetApiError::from)?;

        Ok(res)
    }

    async fn estimate_message_fee(
        &self,
        message: MsgFromL1,
        block_id: BlockIdOrTag,
    ) -> Result<FeeEstimate, Error> {
        let chain_id = self.sequencer.chain_id();

        let tx = message.into_tx_with_chain_id(chain_id);
        let hash = tx.calculate_hash();
        let tx: ExecutableTxWithHash = ExecutableTxWithHash { hash, transaction: tx.into() };

        let res = self
            .sequencer
            .estimate_fee(vec![tx], block_id)
            .map_err(StarknetApiError::from)?
            .pop()
            .expect("should have estimate result");

        Ok(res)
    }

    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTx,
    ) -> Result<DeclareTxResult, Error> {
        if declare_transaction.is_query() {
            return Err(StarknetApiError::UnsupportedTransactionVersion.into());
        }

        let chain_id = self.sequencer.chain_id();

        // // validate compiled class hash
        // let is_valid = declare_transaction
        //     .validate_compiled_class_hash()
        //     .map_err(|_| StarknetApiError::InvalidContractClass)?;

        // if !is_valid {
        //     return Err(StarknetApiError::CompiledClassHashMismatch.into());
        // }

        let tx = declare_transaction
            .try_into_tx_with_chain_id(chain_id)
            .map_err(|_| StarknetApiError::InvalidContractClass)?;

        let class_hash = tx.class_hash();
        let tx = ExecutableTxWithHash::new(ExecutableTx::Declare(tx));
        let tx_hash = tx.hash;

        self.sequencer.add_transaction_to_pool(tx);

        Ok((tx_hash, class_hash).into())
    }

    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTx,
    ) -> Result<InvokeTxResult, Error> {
        if invoke_transaction.is_query {
            return Err(StarknetApiError::UnsupportedTransactionVersion.into());
        }

        let chain_id = self.sequencer.chain_id();

        let tx = invoke_transaction.into_tx_with_chain_id(chain_id);
        let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(tx));
        let tx_hash = tx.hash;

        self.sequencer.add_transaction_to_pool(tx);

        Ok(tx_hash.into())
    }

    async fn transaction_status(
        &self,
        transaction_hash: TxHash,
    ) -> Result<TransactionStatus, Error> {
        let provider = self.sequencer.backend.blockchain.provider();

        let tx_status = TransactionStatusProvider::transaction_status(provider, transaction_hash)
            .map_err(StarknetApiError::from)?;

        if let Some(status) = tx_status {
            if let Some(receipt) = ReceiptProvider::receipt_by_hash(provider, transaction_hash)
                .map_err(StarknetApiError::from)?
            {
                let execution_status = if receipt.is_reverted() {
                    TransactionExecutionStatus::Reverted
                } else {
                    TransactionExecutionStatus::Succeeded
                };

                return Ok(match status {
                    FinalityStatus::AcceptedOnL1 => {
                        TransactionStatus::AcceptedOnL1(execution_status)
                    }
                    FinalityStatus::AcceptedOnL2 => {
                        TransactionStatus::AcceptedOnL2(execution_status)
                    }
                });
            }
        }

        let pending_state = self.sequencer.pending_state();
        let state = pending_state.ok_or(StarknetApiError::TxnHashNotFound)?;
        let executed_txs = state.executed_txs.read();

        // attemps to find in the valid transactions list first (executed_txs)
        // if not found, then search in the rejected transactions list (rejected_txs)
        if let Some(is_reverted) = executed_txs
            .iter()
            .find(|(tx, _)| tx.hash == transaction_hash)
            .map(|(_, rct)| rct.receipt.is_reverted())
        {
            let exec_status = if is_reverted {
                TransactionExecutionStatus::Reverted
            } else {
                TransactionExecutionStatus::Succeeded
            };

            Ok(TransactionStatus::AcceptedOnL2(exec_status))
        } else {
            let rejected_txs = state.rejected_txs.read();

            rejected_txs
                .iter()
                .find(|(tx, _)| tx.hash == transaction_hash)
                .map(|_| TransactionStatus::Rejected)
                .ok_or(Error::from(StarknetApiError::TxnHashNotFound))
        }
    }
}
