use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error, RpcResult};
use katana_core::backend::contract::StarknetContract;
use katana_core::sequencer::KatanaSequencer;
use katana_executor::blockifier::utils::EntryPointCall;
use katana_primitives::block::{BlockHashOrNumber, BlockIdOrTag, FinalityStatus, PartialHeader};
use katana_primitives::conversion::rpc::legacy_inner_to_rpc_class;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxHash};
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::FieldElement;
use katana_provider::traits::block::{BlockHashProvider, BlockIdReader, BlockNumberProvider};
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_rpc_api::starknet::StarknetApiServer;
use katana_rpc_types::block::{
    BlockHashAndNumber, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    PendingBlockWithTxHashes, PendingBlockWithTxs,
};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::event::{EventFilterWithPage, EventsPage};
use katana_rpc_types::message::MsgFromL1;
use katana_rpc_types::receipt::{MaybePendingTxReceipt, PendingTxReceipt};
use katana_rpc_types::state_update::StateUpdate;
use katana_rpc_types::transaction::{
    BroadcastedDeclareTx, BroadcastedDeployAccountTx, BroadcastedInvokeTx, BroadcastedTx,
    DeclareTxResult, DeployAccountTxResult, InvokeTxResult, Tx,
};
use katana_rpc_types::{ContractClass, FeeEstimate, FeltAsHex, FunctionCall, SimulationFlags};
use katana_rpc_types_builder::ReceiptBuilder;
use katana_tasks::{BlockingTaskPool, TokioTaskSpawner};
use starknet::core::types::{BlockTag, TransactionExecutionStatus, TransactionStatus};

#[derive(Clone)]
pub struct StarknetApi {
    inner: Arc<StarknetApiInner>,
}

struct StarknetApiInner {
    sequencer: Arc<KatanaSequencer>,
    blocking_task_pool: BlockingTaskPool,
}

impl StarknetApi {
    pub fn new(sequencer: Arc<KatanaSequencer>) -> Self {
        let blocking_task_pool =
            BlockingTaskPool::new().expect("failed to create blocking task pool");
        Self { inner: Arc::new(StarknetApiInner { sequencer, blocking_task_pool }) }
    }

    async fn on_cpu_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        self.inner.blocking_task_pool.spawn(move || func(this)).await.unwrap()
    }

    async fn on_io_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        TokioTaskSpawner::new().unwrap().spawn_blocking(move || func(this)).await.unwrap()
    }
}
#[async_trait]
impl StarknetApiServer for StarknetApi {
    async fn chain_id(&self) -> RpcResult<FeltAsHex> {
        Ok(FieldElement::from(self.inner.sequencer.chain_id()).into())
    }

    async fn nonce(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> RpcResult<FeltAsHex> {
        self.on_io_blocking_task(move |this| {
            let nonce = this
                .inner
                .sequencer
                .nonce_at(block_id, contract_address.into())
                .map_err(StarknetApiError::from)?
                .ok_or(StarknetApiError::ContractNotFound)?;
            Ok(nonce.into())
        })
        .await
    }

    async fn block_number(&self) -> RpcResult<u64> {
        self.on_io_blocking_task(move |this| {
            let block_number =
                this.inner.sequencer.block_number().map_err(StarknetApiError::from)?;
            Ok(block_number)
        })
        .await
    }

    async fn transaction_by_hash(&self, transaction_hash: FieldElement) -> RpcResult<Tx> {
        self.on_io_blocking_task(move |this| {
            let tx = this
                .inner
                .sequencer
                .transaction(&transaction_hash)
                .map_err(StarknetApiError::from)?
                .ok_or(StarknetApiError::TxnHashNotFound)?;
            Ok(tx.into())
        })
        .await
    }

    async fn block_transaction_count(&self, block_id: BlockIdOrTag) -> RpcResult<u64> {
        self.on_io_blocking_task(move |this| {
            let count = this
                .inner
                .sequencer
                .block_tx_count(block_id)
                .map_err(StarknetApiError::from)?
                .ok_or(StarknetApiError::BlockNotFound)?;
            Ok(count)
        })
        .await
    }

    async fn class_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> RpcResult<ContractClass> {
        let class_hash = self
            .on_io_blocking_task(move |this| {
                this.inner
                    .sequencer
                    .class_hash_at(block_id, contract_address.into())
                    .map_err(StarknetApiError::from)?
                    .ok_or(StarknetApiError::ContractNotFound)
            })
            .await?;
        self.class(block_id, class_hash).await
    }

    async fn block_hash_and_number(&self) -> RpcResult<BlockHashAndNumber> {
        let hash_and_num_pair = self
            .on_io_blocking_task(move |this| this.inner.sequencer.block_hash_and_number())
            .await
            .map_err(StarknetApiError::from)?;
        Ok(hash_and_num_pair.into())
    }

    async fn block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithTxHashes> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.sequencer.backend.blockchain.provider();

            if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                if let Some(pending_state) = this.inner.sequencer.pending_state() {
                    let block_env = pending_state.block_envs.read().0.clone();
                    let latest_hash =
                        BlockHashProvider::latest_hash(provider).map_err(StarknetApiError::from)?;

                    let gas_prices = block_env.l1_gas_prices.clone();

                    let header = PartialHeader {
                        number: block_env.number,
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

                    return Ok(MaybePendingBlockWithTxHashes::Pending(
                        PendingBlockWithTxHashes::new(header, transactions),
                    ));
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
        })
        .await
    }

    async fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> RpcResult<Tx> {
        self.on_io_blocking_task(move |this| {
            // TEMP: have to handle pending tag independently for now
            let tx = if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                let Some(pending_state) = this.inner.sequencer.pending_state() else {
                    return Err(StarknetApiError::BlockNotFound.into());
                };

                let pending_txs = pending_state.executed_txs.read();
                pending_txs.iter().nth(index as usize).map(|(tx, _)| tx.clone())
            } else {
                let provider = &this.inner.sequencer.backend.blockchain.provider();

                let block_num = BlockIdReader::convert_block_id(provider, block_id)
                    .map_err(StarknetApiError::from)?
                    .map(BlockHashOrNumber::Num)
                    .ok_or(StarknetApiError::BlockNotFound)?;

                TransactionProvider::transaction_by_block_and_idx(provider, block_num, index)
                    .map_err(StarknetApiError::from)?
            };

            Ok(tx.ok_or(StarknetApiError::InvalidTxnIndex)?.into())
        })
        .await
    }

    async fn block_with_txs(&self, block_id: BlockIdOrTag) -> RpcResult<MaybePendingBlockWithTxs> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.sequencer.backend.blockchain.provider();

            if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                if let Some(pending_state) = this.inner.sequencer.pending_state() {
                    let block_env = pending_state.block_envs.read().0.clone();
                    let latest_hash =
                        BlockHashProvider::latest_hash(provider).map_err(StarknetApiError::from)?;

                    let gas_prices = block_env.l1_gas_prices.clone();

                    let header = PartialHeader {
                        number: block_env.number,
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
        })
        .await
    }

    async fn state_update(&self, block_id: BlockIdOrTag) -> RpcResult<StateUpdate> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.sequencer.backend.blockchain.provider();

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
        })
        .await
    }

    async fn transaction_receipt(
        &self,
        transaction_hash: FieldElement,
    ) -> RpcResult<MaybePendingTxReceipt> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.sequencer.backend.blockchain.provider();
            let receipt = ReceiptBuilder::new(transaction_hash, provider)
                .build()
                .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?;

            match receipt {
                Some(receipt) => Ok(MaybePendingTxReceipt::Receipt(receipt)),

                None => {
                    let pending_receipt = this.inner.sequencer.pending_state().and_then(|s| {
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
        })
        .await
    }

    async fn class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: FieldElement,
    ) -> RpcResult<FeltAsHex> {
        self.on_io_blocking_task(move |this| {
            let hash = this
                .inner
                .sequencer
                .class_hash_at(block_id, contract_address.into())
                .map_err(StarknetApiError::from)?
                .ok_or(StarknetApiError::ContractNotFound)?;
            Ok(hash.into())
        })
        .await
    }

    async fn class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: FieldElement,
    ) -> RpcResult<ContractClass> {
        self.on_io_blocking_task(move |this| {
            let class =
                this.inner.sequencer.class(block_id, class_hash).map_err(StarknetApiError::from)?;
            let Some(class) = class else { return Err(StarknetApiError::ClassHashNotFound.into()) };

            match class {
                StarknetContract::Legacy(c) => {
                    let contract = legacy_inner_to_rpc_class(c)
                        .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?;
                    Ok(contract)
                }
                StarknetContract::Sierra(c) => Ok(ContractClass::Sierra(c)),
            }
        })
        .await
    }

    async fn events(&self, filter: EventFilterWithPage) -> RpcResult<EventsPage> {
        self.on_io_blocking_task(move |this| {
            let from_block = filter.event_filter.from_block.unwrap_or(BlockIdOrTag::Number(0));
            let to_block =
                filter.event_filter.to_block.unwrap_or(BlockIdOrTag::Tag(BlockTag::Latest));

            let keys = filter.event_filter.keys;
            let keys = keys.filter(|keys| !(keys.len() == 1 && keys.is_empty()));

            let events = this
                .inner
                .sequencer
                .events(
                    from_block,
                    to_block,
                    filter.event_filter.address.map(|f| f.into()),
                    keys,
                    filter.result_page_request.continuation_token,
                    filter.result_page_request.chunk_size,
                )
                .map_err(StarknetApiError::from)?;

            Ok(events)
        })
        .await
    }

    async fn call(
        &self,
        request: FunctionCall,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<FeltAsHex>> {
        self.on_io_blocking_task(move |this| {
            let request = EntryPointCall {
                calldata: request.calldata,
                contract_address: request.contract_address.into(),
                entry_point_selector: request.entry_point_selector,
            };

            let res =
                this.inner.sequencer.call(request, block_id).map_err(StarknetApiError::from)?;
            Ok(res.into_iter().map(|v| v.into()).collect())
        })
        .await
    }

    async fn storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        block_id: BlockIdOrTag,
    ) -> RpcResult<FeltAsHex> {
        self.on_io_blocking_task(move |this| {
            let value = this
                .inner
                .sequencer
                .storage_at(contract_address.into(), key, block_id)
                .map_err(StarknetApiError::from)?;

            Ok(value.into())
        })
        .await
    }

    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTx,
    ) -> RpcResult<DeployAccountTxResult> {
        self.on_io_blocking_task(move |this| {
            if deploy_account_transaction.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion.into());
            }

            let chain_id = this.inner.sequencer.chain_id();

            let tx = deploy_account_transaction.into_tx_with_chain_id(chain_id);
            let contract_address = tx.contract_address();

            let tx = ExecutableTxWithHash::new(ExecutableTx::DeployAccount(tx));
            let tx_hash = tx.hash;

            this.inner.sequencer.add_transaction_to_pool(tx);

            Ok((tx_hash, contract_address).into())
        })
        .await
    }

    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTx>,
        simulation_flags: Vec<SimulationFlags>,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<FeeEstimate>> {
        self.on_cpu_blocking_task(move |this| {
            let chain_id = this.inner.sequencer.chain_id();

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

            let skip_validate =
                simulation_flags.iter().any(|flag| flag == &SimulationFlags::SkipValidate);

            let res = this
                .inner
                .sequencer
                .estimate_fee(transactions, block_id, skip_validate)
                .map_err(StarknetApiError::from)?;

            Ok(res)
        })
        .await
    }

    async fn estimate_message_fee(
        &self,
        message: MsgFromL1,
        block_id: BlockIdOrTag,
    ) -> RpcResult<FeeEstimate> {
        self.on_cpu_blocking_task(move |this| {
            let chain_id = this.inner.sequencer.chain_id();

            let tx = message.into_tx_with_chain_id(chain_id);
            let hash = tx.calculate_hash();
            let tx: ExecutableTxWithHash = ExecutableTxWithHash { hash, transaction: tx.into() };

            let res = this
                .inner
                .sequencer
                .estimate_fee(vec![tx], block_id, false)
                .map_err(StarknetApiError::from)?
                .pop()
                .expect("should have estimate result");

            Ok(res)
        })
        .await
    }

    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTx,
    ) -> RpcResult<DeclareTxResult> {
        self.on_io_blocking_task(move |this| {
            if declare_transaction.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion.into());
            }

            let chain_id = this.inner.sequencer.chain_id();

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

            this.inner.sequencer.add_transaction_to_pool(tx);

            Ok((tx_hash, class_hash).into())
        })
        .await
    }

    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTx,
    ) -> RpcResult<InvokeTxResult> {
        self.on_io_blocking_task(move |this| {
            if invoke_transaction.is_query() {
                return Err(StarknetApiError::UnsupportedTransactionVersion.into());
            }

            let chain_id = this.inner.sequencer.chain_id();

            let tx = invoke_transaction.into_tx_with_chain_id(chain_id);
            let tx = ExecutableTxWithHash::new(ExecutableTx::Invoke(tx));
            let tx_hash = tx.hash;

            this.inner.sequencer.add_transaction_to_pool(tx);

            Ok(tx_hash.into())
        })
        .await
    }

    async fn transaction_status(&self, transaction_hash: TxHash) -> RpcResult<TransactionStatus> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.sequencer.backend.blockchain.provider();

            let tx_status =
                TransactionStatusProvider::transaction_status(provider, transaction_hash)
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

            let pending_state = this.inner.sequencer.pending_state();
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
        })
        .await
    }
}
