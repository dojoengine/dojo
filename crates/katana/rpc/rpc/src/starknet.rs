use std::sync::Arc;

use jsonrpsee::core::{async_trait, Error, RpcResult};
use katana_core::backend::contract::StarknetContract;
use katana_core::sequencer::KatanaSequencer;
use katana_executor::{EntryPointCall, ExecutionResult, ExecutorFactory, ResultAndStates};
use katana_primitives::block::{BlockHashOrNumber, BlockIdOrTag, FinalityStatus, PartialHeader};
use katana_primitives::conversion::rpc::legacy_inner_to_rpc_class;
use katana_primitives::receipt::Receipt;
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
use katana_rpc_types::trace::FunctionInvocation;
use katana_rpc_types::transaction::{
    BroadcastedDeclareTx, BroadcastedDeployAccountTx, BroadcastedInvokeTx, BroadcastedTx,
    DeclareTxResult, DeployAccountTxResult, InvokeTxResult, Tx,
};
use katana_rpc_types::{
    ContractClass, FeeEstimate, FeltAsHex, FunctionCall, SimulationFlag,
    SimulationFlagForEstimateFee,
};
use katana_rpc_types_builder::ReceiptBuilder;
use katana_tasks::{BlockingTaskPool, TokioTaskSpawner};
use starknet::core::types::{
    BlockTag, DeclareTransactionTrace, DeployAccountTransactionTrace, ExecuteInvocation,
    InvokeTransactionTrace, L1HandlerTransactionTrace, RevertedInvocation, SimulatedTransaction,
    TransactionExecutionStatus, TransactionStatus, TransactionTrace,
};

pub struct StarknetApi<EF: ExecutorFactory> {
    inner: Arc<StarknetApiInner<EF>>,
}

impl<EF: ExecutorFactory> Clone for StarknetApi<EF> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

struct StarknetApiInner<EF: ExecutorFactory> {
    sequencer: Arc<KatanaSequencer<EF>>,
    blocking_task_pool: BlockingTaskPool,
}

impl<EF: ExecutorFactory> StarknetApi<EF> {
    pub fn new(sequencer: Arc<KatanaSequencer<EF>>) -> Self {
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

    fn estimate_fee_with(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        block_id: BlockIdOrTag,
        flags: katana_executor::SimulationFlag,
    ) -> Result<Vec<FeeEstimate>, StarknetApiError> {
        let sequencer = &self.inner.sequencer;
        // get the state and block env at the specified block for execution
        let state = sequencer.state(&block_id).map_err(StarknetApiError::from)?;
        let env = sequencer
            .block_env_at(block_id)
            .map_err(StarknetApiError::from)?
            .ok_or(StarknetApiError::BlockNotFound)?;

        // create the executor
        let executor = sequencer.backend.executor_factory.with_state_and_block_env(state, env);
        let results = executor.estimate_fee(transactions, flags);

        let mut estimates = Vec::with_capacity(results.len());
        for (i, res) in results.into_iter().enumerate() {
            match res {
                Ok(fee) => estimates.push(FeeEstimate {
                    gas_price: fee.gas_price.into(),
                    gas_consumed: fee.gas_consumed.into(),
                    overall_fee: fee.overall_fee.into(),
                    unit: fee.unit,
                }),

                Err(err) => {
                    return Err(StarknetApiError::TransactionExecutionError {
                        transaction_index: i,
                        execution_error: err.to_string(),
                    });
                }
            }
        }

        Ok(estimates)
    }
}

#[async_trait]
impl<EF: ExecutorFactory> StarknetApiServer for StarknetApi<EF> {
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
                if let Some(executor) = this.inner.sequencer.pending_executor() {
                    let block_env = executor.read().block_env();
                    let latest_hash = provider.latest_hash().map_err(StarknetApiError::from)?;

                    let gas_prices = block_env.l1_gas_prices.clone();

                    let header = PartialHeader {
                        number: block_env.number,
                        gas_prices,
                        parent_hash: latest_hash,
                        timestamp: block_env.timestamp,
                        version: CURRENT_STARKNET_VERSION,
                        sequencer_address: block_env.sequencer_address,
                    };

                    // TODO(kariy): create a method that can perform this filtering for us instead
                    // of doing it manually.

                    // A block should only include successful transactions, we filter out the failed
                    // ones (didn't pass validation stage).
                    let transactions = executor
                        .read()
                        .transactions()
                        .iter()
                        .filter(|(_, receipt)| receipt.is_success())
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
                let Some(executor) = this.inner.sequencer.pending_executor() else {
                    return Err(StarknetApiError::BlockNotFound.into());
                };

                let executor = executor.read();
                let pending_txs = executor.transactions();
                pending_txs.get(index as usize).map(|(tx, _)| tx.clone())
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
                if let Some(executor) = this.inner.sequencer.pending_executor() {
                    let block_env = executor.read().block_env();
                    let latest_hash = provider.latest_hash().map_err(StarknetApiError::from)?;

                    let gas_prices = block_env.l1_gas_prices.clone();

                    let header = PartialHeader {
                        number: block_env.number,
                        gas_prices,
                        parent_hash: latest_hash,
                        version: CURRENT_STARKNET_VERSION,
                        timestamp: block_env.timestamp,
                        sequencer_address: block_env.sequencer_address,
                    };

                    // TODO(kariy): create a method that can perform this filtering for us instead
                    // of doing it manually.

                    // A block should only include successful transactions, we filter out the failed
                    // ones (didn't pass validation stage).
                    let transactions = executor
                        .read()
                        .transactions()
                        .iter()
                        .filter(|(_, receipt)| receipt.is_success())
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
                    let executor = this.inner.sequencer.pending_executor();
                    let pending_receipt = executor
                        .and_then(|executor| {
                            executor.read().transactions().iter().find_map(|(tx, res)| {
                                if tx.hash == transaction_hash {
                                    match res {
                                        ExecutionResult::Failed { .. } => None,
                                        ExecutionResult::Success { receipt, .. } => {
                                            Some(receipt.clone())
                                        }
                                    }
                                } else {
                                    None
                                }
                            })
                        })
                        .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;

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

            let sequencer = &this.inner.sequencer;

            // get the state and block env at the specified block for function call execution
            let state = sequencer.state(&block_id).map_err(StarknetApiError::from)?;
            let env = sequencer
                .block_env_at(block_id)
                .map_err(StarknetApiError::from)?
                .ok_or(StarknetApiError::BlockNotFound)?;

            let executor = sequencer.backend.executor_factory.with_state_and_block_env(state, env);

            match executor.call(request) {
                Ok(retdata) => Ok(retdata.into_iter().map(|v| v.into()).collect()),
                Err(err) => Err(Error::from(StarknetApiError::ContractError {
                    revert_error: err.to_string(),
                })),
            }
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
        simulation_flags: Vec<SimulationFlagForEstimateFee>,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<FeeEstimate>> {
        self.on_cpu_blocking_task(move |this| {
            let sequencer = &this.inner.sequencer;
            let chain_id = sequencer.chain_id();

            let transactions = request
                .into_iter()
                .map(|tx| {
                    let tx = match tx {
                        BroadcastedTx::Invoke(tx) => {
                            let is_query = tx.is_query();
                            let tx = tx.into_tx_with_chain_id(chain_id);
                            ExecutableTxWithHash::new_query(ExecutableTx::Invoke(tx), is_query)
                        }

                        BroadcastedTx::DeployAccount(tx) => {
                            let is_query = tx.is_query();
                            let tx = tx.into_tx_with_chain_id(chain_id);
                            ExecutableTxWithHash::new_query(
                                ExecutableTx::DeployAccount(tx),
                                is_query,
                            )
                        }

                        BroadcastedTx::Declare(tx) => {
                            let is_query = tx.is_query();
                            let tx = tx
                                .try_into_tx_with_chain_id(chain_id)
                                .map_err(|_| StarknetApiError::InvalidContractClass)?;
                            ExecutableTxWithHash::new_query(ExecutableTx::Declare(tx), is_query)
                        }
                    };

                    Result::<ExecutableTxWithHash, StarknetApiError>::Ok(tx)
                })
                .collect::<Result<Vec<_>, _>>()?;

            let skip_validate =
                simulation_flags.contains(&SimulationFlagForEstimateFee::SkipValidate);

            // If the node is run with transaction validation disabled, then we should not validate
            // transactions when estimating the fee even if the `SKIP_VALIDATE` flag is not set.
            let should_validate =
                !(skip_validate || this.inner.sequencer.backend.config.disable_validate);
            let flags = katana_executor::SimulationFlag {
                skip_validate: !should_validate,
                ..Default::default()
            };

            let results = this.estimate_fee_with(transactions, block_id, flags)?;
            Ok(results)
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

            let result = this.estimate_fee_with(
                vec![ExecutableTxWithHash { hash, transaction: tx.into() }],
                block_id,
                Default::default(),
            );
            match result {
                Ok(mut res) => {
                    if let Some(fee) = res.pop() {
                        Ok(fee)
                    } else {
                        Err(Error::from(StarknetApiError::UnexpectedError {
                            reason: "Fee estimation result should exist".into(),
                        }))
                    }
                }

                Err(err) => Err(Error::from(err)),
            }
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

            let pending_executor =
                this.inner.sequencer.pending_executor().ok_or(StarknetApiError::TxnHashNotFound)?;
            let pending_executor = pending_executor.read();

            let pending_txs = pending_executor.transactions();
            let status =
                pending_txs.iter().find(|(tx, _)| tx.hash == transaction_hash).map(|(_, res)| {
                    match res {
                        ExecutionResult::Failed { .. } => TransactionStatus::Rejected,
                        ExecutionResult::Success { receipt, .. } => {
                            TransactionStatus::AcceptedOnL2(if receipt.is_reverted() {
                                TransactionExecutionStatus::Reverted
                            } else {
                                TransactionExecutionStatus::Succeeded
                            })
                        }
                    }
                });

            status.ok_or(Error::from(StarknetApiError::TxnHashNotFound))
        })
        .await
    }

    async fn simulate_transactions(
        &self,
        block_id: BlockIdOrTag,
        transactions: Vec<BroadcastedTx>,
        simulation_flags: Vec<SimulationFlag>,
    ) -> RpcResult<Vec<SimulatedTransaction>> {
        self.on_cpu_blocking_task(move |this| {
            let chain_id = this.inner.sequencer.chain_id();

            let executables = transactions
                .into_iter()
                .map(|tx| {
                    let tx = match tx {
                        BroadcastedTx::Invoke(tx) => {
                            let is_query = tx.is_query();
                            ExecutableTxWithHash::new_query(
                                ExecutableTx::Invoke(tx.into_tx_with_chain_id(chain_id)),
                                is_query,
                            )
                        }
                        BroadcastedTx::Declare(tx) => {
                            let is_query = tx.is_query();
                            ExecutableTxWithHash::new_query(
                                ExecutableTx::Declare(
                                    tx.try_into_tx_with_chain_id(chain_id)
                                        .map_err(|_| StarknetApiError::InvalidContractClass)?,
                                ),
                                is_query,
                            )
                        }
                        BroadcastedTx::DeployAccount(tx) => {
                            let is_query = tx.is_query();
                            ExecutableTxWithHash::new_query(
                                ExecutableTx::DeployAccount(tx.into_tx_with_chain_id(chain_id)),
                                is_query,
                            )
                        }
                    };
                    Result::<ExecutableTxWithHash, StarknetApiError>::Ok(tx)
                })
                .collect::<Result<Vec<_>, _>>()?;

            // If the node is run with transaction validation disabled, then we should not validate
            // even if the `SKIP_VALIDATE` flag is not set.
            let should_validate = !(simulation_flags.contains(&SimulationFlag::SkipValidate)
                || this.inner.sequencer.backend.config.disable_validate);
            // If the node is run with fee charge disabled, then we should disable charing fees even
            // if the `SKIP_FEE_CHARGE` flag is not set.
            let should_skip_fee = !(simulation_flags.contains(&SimulationFlag::SkipFeeCharge)
                || this.inner.sequencer.backend.config.disable_fee);

            let flags = katana_executor::SimulationFlag {
                skip_validate: !should_validate,
                skip_fee_transfer: !should_skip_fee,
                ..Default::default()
            };

            let sequencer = &this.inner.sequencer;
            // get the state and block env at the specified block for execution
            let state = sequencer.state(&block_id).map_err(StarknetApiError::from)?;
            let env = sequencer
                .block_env_at(block_id)
                .map_err(StarknetApiError::from)?
                .ok_or(StarknetApiError::BlockNotFound)?;

            // create the executor
            let executor = sequencer.backend.executor_factory.with_state_and_block_env(state, env);
            let results = executor.simulate(executables, flags);

            let mut simulated = Vec::with_capacity(results.len());
            for (i, ResultAndStates { result, .. }) in results.into_iter().enumerate() {
                match result {
                    ExecutionResult::Success { trace, receipt } => {
                        let fee_transfer_invocation =
                            trace.fee_transfer_call_info.map(|f| FunctionInvocation::from(f).0);
                        let validate_invocation =
                            trace.validate_call_info.map(|f| FunctionInvocation::from(f).0);
                        let execute_invocation =
                            trace.execute_call_info.map(|f| FunctionInvocation::from(f).0);
                        let revert_reason = trace.revert_error;
                        // TODO: compute the state diff
                        let state_diff = None;

                        let transaction_trace = match receipt {
                            Receipt::Invoke(_) => {
                                TransactionTrace::Invoke(InvokeTransactionTrace {
                                    fee_transfer_invocation,
                                    validate_invocation,
                                    state_diff,
                                    execute_invocation: if let Some(revert_reason) = revert_reason {
                                        ExecuteInvocation::Reverted(RevertedInvocation {
                                            revert_reason,
                                        })
                                    } else {
                                        ExecuteInvocation::Success(
                                            execute_invocation
                                                .expect("should exist if not reverted"),
                                        )
                                    },
                                })
                            }

                            Receipt::Declare(_) => {
                                TransactionTrace::Declare(DeclareTransactionTrace {
                                    fee_transfer_invocation,
                                    validate_invocation,
                                    state_diff,
                                })
                            }

                            Receipt::DeployAccount(_) => {
                                TransactionTrace::DeployAccount(DeployAccountTransactionTrace {
                                    fee_transfer_invocation,
                                    validate_invocation,
                                    state_diff,
                                    constructor_invocation: execute_invocation
                                        .expect("should exist bcs tx succeed"),
                                })
                            }

                            Receipt::L1Handler(_) => {
                                TransactionTrace::L1Handler(L1HandlerTransactionTrace {
                                    state_diff,
                                    function_invocation: execute_invocation
                                        .expect("should exist bcs tx succeed"),
                                })
                            }
                        };

                        let fee = receipt.fee();
                        simulated.push(SimulatedTransaction {
                            transaction_trace,
                            fee_estimation: FeeEstimate {
                                unit: fee.unit,
                                gas_price: fee.gas_price.into(),
                                overall_fee: fee.overall_fee.into(),
                                gas_consumed: fee.gas_consumed.into(),
                            },
                        })
                    }

                    ExecutionResult::Failed { error } => {
                        return Err(Error::from(StarknetApiError::TransactionExecutionError {
                            transaction_index: i,
                            execution_error: error.to_string(),
                        }));
                    }
                }
            }

            Ok(simulated)
        })
        .await
    }
}
