//! Server implementation for the Starknet JSON-RPC API.

pub mod forking;
mod read;
mod trace;
mod write;

use std::sync::Arc;

use forking::ForkedClient;
use katana_core::backend::Backend;
use katana_core::service::block_producer::{BlockProducer, BlockProducerMode, PendingExecutor};
use katana_executor::{ExecutionResult, ExecutorFactory};
use katana_pool::validation::stateful::TxValidator;
use katana_pool::TxPool;
use katana_primitives::block::{
    BlockHash, BlockHashOrNumber, BlockIdOrTag, BlockNumber, BlockTag, FinalityStatus,
    PartialHeader,
};
use katana_primitives::class::{ClassHash, CompiledClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::conversion::rpc::legacy_inner_to_rpc_class;
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::env::BlockEnv;
use katana_primitives::event::ContinuationToken;
use katana_primitives::transaction::{ExecutableTxWithHash, TxHash};
use katana_primitives::Felt;
use katana_provider::traits::block::{BlockHashProvider, BlockIdReader, BlockNumberProvider};
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_rpc_types::block::{
    MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    PendingBlockWithReceipts, PendingBlockWithTxHashes, PendingBlockWithTxs,
};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::receipt::{ReceiptBlock, TxReceiptWithBlockInfo};
use katana_rpc_types::state_update::MaybePendingStateUpdate;
use katana_rpc_types::transaction::Tx;
use katana_rpc_types::FeeEstimate;
use katana_rpc_types_builder::ReceiptBuilder;
use katana_tasks::{BlockingTaskPool, TokioTaskSpawner};
use starknet::core::types::{
    ContractClass, EventsPage, PriceUnit, TransactionExecutionStatus, TransactionStatus,
};

use crate::utils;
use crate::utils::events::{Cursor, EventBlockId};

pub type StarknetApiResult<T> = Result<T, StarknetApiError>;

#[allow(missing_debug_implementations)]
pub struct StarknetApi<EF: ExecutorFactory> {
    inner: Arc<Inner<EF>>,
}

impl<EF: ExecutorFactory> Clone for StarknetApi<EF> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

struct Inner<EF: ExecutorFactory> {
    validator: TxValidator,
    pool: TxPool,
    backend: Arc<Backend<EF>>,
    block_producer: BlockProducer<EF>,
    blocking_task_pool: BlockingTaskPool,
    forked_client: Option<ForkedClient>,
}

impl<EF: ExecutorFactory> StarknetApi<EF> {
    pub fn new(
        backend: Arc<Backend<EF>>,
        pool: TxPool,
        block_producer: BlockProducer<EF>,
        validator: TxValidator,
    ) -> Self {
        Self::new_inner(backend, pool, block_producer, validator, None)
    }

    pub fn new_forked(
        backend: Arc<Backend<EF>>,
        pool: TxPool,
        block_producer: BlockProducer<EF>,
        validator: TxValidator,
        forked_client: ForkedClient,
    ) -> Self {
        Self::new_inner(backend, pool, block_producer, validator, Some(forked_client))
    }

    fn new_inner(
        backend: Arc<Backend<EF>>,
        pool: TxPool,
        block_producer: BlockProducer<EF>,
        validator: TxValidator,
        forked_client: Option<ForkedClient>,
    ) -> Self {
        let blocking_task_pool =
            BlockingTaskPool::new().expect("failed to create blocking task pool");
        let inner =
            Inner { pool, backend, block_producer, blocking_task_pool, validator, forked_client };
        Self { inner: Arc::new(inner) }
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
        flags: katana_executor::ExecutionFlags,
    ) -> StarknetApiResult<Vec<FeeEstimate>> {
        // get the state and block env at the specified block for execution
        let state = self.state(&block_id)?;
        let env = self.block_env_at(&block_id)?;

        // create the executor
        let executor = self.inner.backend.executor_factory.with_state_and_block_env(state, env);
        let results = executor.estimate_fee(transactions, flags);

        let mut estimates = Vec::with_capacity(results.len());
        for (i, res) in results.into_iter().enumerate() {
            match res {
                Ok(fee) => estimates.push(FeeEstimate {
                    gas_price: fee.gas_price.into(),
                    gas_consumed: fee.gas_consumed.into(),
                    overall_fee: fee.overall_fee.into(),
                    data_gas_price: Default::default(),
                    data_gas_consumed: Default::default(),
                    unit: match fee.unit {
                        katana_primitives::fee::PriceUnit::Wei => PriceUnit::Wei,
                        katana_primitives::fee::PriceUnit::Fri => PriceUnit::Fri,
                    },
                }),

                Err(err) => {
                    return Err(StarknetApiError::TransactionExecutionError {
                        transaction_index: i as u64,
                        execution_error: err.to_string(),
                    });
                }
            }
        }

        Ok(estimates)
    }

    /// Returns the pending state if the sequencer is running in _interval_ mode. Otherwise `None`.
    fn pending_executor(&self) -> Option<PendingExecutor> {
        match &*self.inner.block_producer.producer.read() {
            BlockProducerMode::Instant(_) => None,
            BlockProducerMode::Interval(producer) => Some(producer.executor()),
        }
    }

    fn state(&self, block_id: &BlockIdOrTag) -> StarknetApiResult<Box<dyn StateProvider>> {
        let provider = self.inner.backend.blockchain.provider();

        let state = match block_id {
            BlockIdOrTag::Tag(BlockTag::Latest) => Some(provider.latest()?),

            BlockIdOrTag::Tag(BlockTag::Pending) => {
                if let Some(exec) = self.pending_executor() {
                    Some(exec.read().state())
                } else {
                    Some(provider.latest()?)
                }
            }

            BlockIdOrTag::Hash(hash) => provider.historical((*hash).into())?,
            BlockIdOrTag::Number(num) => provider.historical((*num).into())?,
        };

        state.ok_or(StarknetApiError::BlockNotFound)
    }

    fn block_env_at(&self, block_id: &BlockIdOrTag) -> StarknetApiResult<BlockEnv> {
        let provider = self.inner.backend.blockchain.provider();

        let env = match block_id {
            BlockIdOrTag::Tag(BlockTag::Pending) => {
                if let Some(exec) = self.pending_executor() {
                    Some(exec.read().block_env())
                } else {
                    let num = provider.latest_number()?;
                    provider.block_env_at(num.into())?
                }
            }

            BlockIdOrTag::Tag(BlockTag::Latest) => {
                let num = provider.latest_number()?;
                provider.block_env_at(num.into())?
            }

            BlockIdOrTag::Hash(hash) => provider.block_env_at((*hash).into())?,
            BlockIdOrTag::Number(num) => provider.block_env_at((*num).into())?,
        };

        env.ok_or(StarknetApiError::BlockNotFound)
    }

    fn block_hash_and_number(&self) -> StarknetApiResult<(BlockHash, BlockNumber)> {
        let provider = self.inner.backend.blockchain.provider();
        let hash = provider.latest_hash()?;
        let number = provider.latest_number()?;
        Ok((hash, number))
    }

    async fn class_at_hash(
        &self,
        block_id: BlockIdOrTag,
        class_hash: ClassHash,
    ) -> StarknetApiResult<ContractClass> {
        self.on_io_blocking_task(move |this| {
            let state = this.state(&block_id)?;

            let Some(class) = state.class(class_hash)? else {
                return Err(StarknetApiError::ClassHashNotFound);
            };

            match class {
                CompiledClass::Deprecated(class) => Ok(legacy_inner_to_rpc_class(class)?),
                CompiledClass::Class(_) => {
                    let Some(sierra) = state.sierra_class(class_hash)? else {
                        return Err(StarknetApiError::UnexpectedError {
                            reason: "Class hash exist, but its Sierra class is missing".to_string(),
                        });
                    };

                    Ok(ContractClass::Sierra(sierra))
                }
            }
        })
        .await
    }

    async fn class_hash_at_address(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> StarknetApiResult<ClassHash> {
        self.on_io_blocking_task(move |this| {
            let state = this.state(&block_id)?;
            let class_hash = state.class_hash_of_contract(contract_address)?;
            class_hash.ok_or(StarknetApiError::ContractNotFound)
        })
        .await
    }

    async fn class_at_address(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> StarknetApiResult<ContractClass> {
        let hash = self.class_hash_at_address(block_id, contract_address).await?;
        let class = self.class_at_hash(block_id, hash).await?;
        Ok(class)
    }

    fn storage_at(
        &self,
        contract_address: ContractAddress,
        storage_key: StorageKey,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<StorageValue> {
        let state = self.state(&block_id)?;

        // check that contract exist by checking the class hash of the contract
        let Some(_) = state.class_hash_of_contract(contract_address)? else {
            return Err(StarknetApiError::ContractNotFound);
        };

        let value = state.storage(contract_address, storage_key)?;
        Ok(value.unwrap_or_default())
    }

    async fn block_tx_count(&self, block_id: BlockIdOrTag) -> StarknetApiResult<u64> {
        let count = self
            .on_io_blocking_task(move |this| {
                let provider = this.inner.backend.blockchain.provider();

                let block_id: BlockHashOrNumber = match block_id {
                    BlockIdOrTag::Tag(BlockTag::Pending) => match this.pending_executor() {
                        Some(exec) => {
                            let count = exec.read().transactions().len() as u64;
                            return Ok(Some(count));
                        }
                        None => provider.latest_hash()?.into(),
                    },
                    BlockIdOrTag::Tag(BlockTag::Latest) => provider.latest_number()?.into(),
                    BlockIdOrTag::Number(num) => num.into(),
                    BlockIdOrTag::Hash(hash) => hash.into(),
                };

                let count = provider.transaction_count_by_block(block_id)?;
                Result::<_, StarknetApiError>::Ok(count)
            })
            .await?;

        if let Some(count) = count {
            Ok(count)
        } else if let Some(client) = &self.inner.forked_client {
            let status = client.get_block_transaction_count(block_id).await?;
            Ok(status)
        } else {
            Err(StarknetApiError::BlockNotFound)
        }
    }

    async fn latest_block_number(&self) -> StarknetApiResult<BlockNumber> {
        self.on_io_blocking_task(move |this| {
            Ok(this.inner.backend.blockchain.provider().latest_number()?)
        })
        .await
    }

    async fn nonce_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: ContractAddress,
    ) -> StarknetApiResult<Nonce> {
        self.on_io_blocking_task(move |this| {
            // read from the pool state if pending block
            //
            // TODO: this is a temporary solution, we should have a better way to handle this.
            // perhaps a pending/pool state provider that implements all the state provider traits.
            let result = if let BlockIdOrTag::Tag(BlockTag::Pending) = block_id {
                this.inner.validator.pool_nonce(contract_address)?
            } else {
                let state = this.state(&block_id)?;
                state.nonce(contract_address)?
            };

            let nonce = result.ok_or(StarknetApiError::ContractNotFound)?;
            Ok(nonce)
        })
        .await
    }

    async fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> StarknetApiResult<Tx> {
        let tx = self
            .on_io_blocking_task(move |this| {
                // TEMP: have to handle pending tag independently for now
                let tx = if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                    let Some(executor) = this.pending_executor() else {
                        return Err(StarknetApiError::BlockNotFound);
                    };

                    let executor = executor.read();
                    let pending_txs = executor.transactions();
                    pending_txs.get(index as usize).map(|(tx, _)| tx.clone())
                } else {
                    let provider = &this.inner.backend.blockchain.provider();

                    let block_num = BlockIdReader::convert_block_id(provider, block_id)?
                        .map(BlockHashOrNumber::Num)
                        .ok_or(StarknetApiError::BlockNotFound)?;

                    provider.transaction_by_block_and_idx(block_num, index)?
                };

                StarknetApiResult::Ok(tx)
            })
            .await?;

        if let Some(tx) = tx {
            Ok(tx.into())
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_transaction_by_block_id_and_index(block_id, index).await?)
        } else {
            Err(StarknetApiError::InvalidTxnIndex)
        }
    }

    async fn transaction(&self, hash: TxHash) -> StarknetApiResult<Tx> {
        let tx = self
            .on_io_blocking_task(move |this| {
                let tx = this
                    .inner
                    .backend
                    .blockchain
                    .provider()
                    .transaction_by_hash(hash)?
                    .map(Tx::from);

                let result = match tx {
                    tx @ Some(_) => tx,
                    None => {
                        // check if the transaction is in the pending block
                        this.pending_executor().as_ref().and_then(|exec| {
                            exec.read()
                                .transactions()
                                .iter()
                                .find(|(tx, _)| tx.hash == hash)
                                .map(|(tx, _)| Tx::from(tx.clone()))
                        })
                    }
                };

                Result::<_, StarknetApiError>::Ok(result)
            })
            .await?;

        if let Some(tx) = tx {
            Ok(tx)
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_transaction_by_hash(hash).await?)
        } else {
            Err(StarknetApiError::TxnHashNotFound)
        }
    }

    async fn receipt(&self, hash: Felt) -> StarknetApiResult<TxReceiptWithBlockInfo> {
        let receipt = self
            .on_io_blocking_task(move |this| {
                let provider = this.inner.backend.blockchain.provider();
                let receipt = ReceiptBuilder::new(hash, provider).build()?;

                // If receipt is not found, check the pending block.
                match receipt {
                    Some(receipt) => Ok(Some(receipt)),
                    None => {
                        let executor = this.pending_executor();
                        // If there's a pending executor
                        let pending_receipt = executor.and_then(|executor| {
                            // Find the transaction in the pending block that matches the hash
                            executor.read().transactions().iter().find_map(|(tx, res)| {
                                if tx.hash == hash {
                                    // If the transaction is found, only return the receipt if it's
                                    // successful
                                    match res {
                                        ExecutionResult::Success { receipt, .. } => {
                                            Some(receipt.clone())
                                        }
                                        ExecutionResult::Failed { .. } => None,
                                    }
                                } else {
                                    None
                                }
                            })
                        });

                        if let Some(receipt) = pending_receipt {
                            let receipt = TxReceiptWithBlockInfo::new(
                                ReceiptBlock::Pending,
                                hash,
                                FinalityStatus::AcceptedOnL2,
                                receipt,
                            );

                            StarknetApiResult::Ok(Some(receipt))
                        } else {
                            StarknetApiResult::Ok(None)
                        }
                    }
                }
            })
            .await?;

        if let Some(receipt) = receipt {
            Ok(receipt)
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_transaction_receipt(hash).await?)
        } else {
            Err(StarknetApiError::TxnHashNotFound)
        }
    }

    // TODO: should document more and possible find a simpler solution(?)
    fn events(
        &self,
        from_block: BlockIdOrTag,
        to_block: BlockIdOrTag,
        address: Option<ContractAddress>,
        keys: Option<Vec<Vec<Felt>>>,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> StarknetApiResult<EventsPage> {
        let provider = self.inner.backend.blockchain.provider();

        let from = if BlockIdOrTag::Tag(BlockTag::Pending) == from_block {
            EventBlockId::Pending
        } else {
            let num = provider.convert_block_id(from_block)?;
            EventBlockId::Num(num.ok_or(StarknetApiError::BlockNotFound)?)
        };

        let to = if BlockIdOrTag::Tag(BlockTag::Pending) == to_block {
            EventBlockId::Pending
        } else {
            let num = provider.convert_block_id(to_block)?;
            EventBlockId::Num(num.ok_or(StarknetApiError::BlockNotFound)?)
        };

        let token: Option<Cursor> = match continuation_token {
            Some(token) => Some(ContinuationToken::parse(&token)?.into()),
            None => None,
        };

        // reserved buffer to fill up with events to avoid reallocations
        let mut buffer = Vec::with_capacity(chunk_size as usize);
        let filter = utils::events::Filter { address, keys };

        match (from, to) {
            (EventBlockId::Num(from), EventBlockId::Num(to)) => {
                let cursor = utils::events::fetch_events_at_blocks(
                    provider,
                    from..=to,
                    &filter,
                    chunk_size,
                    token,
                    &mut buffer,
                )?;

                Ok(EventsPage {
                    events: buffer,
                    continuation_token: cursor.map(|c| c.into_rpc_cursor().to_string()),
                })
            }

            (EventBlockId::Num(from), EventBlockId::Pending) => {
                let latest = provider.latest_number()?;
                let int_cursor = utils::events::fetch_events_at_blocks(
                    provider,
                    from..=latest,
                    &filter,
                    chunk_size,
                    token.clone(),
                    &mut buffer,
                )?;

                // if the internal cursor is Some, meaning the buffer is full and we havent
                // reached the latest block.
                if let Some(c) = int_cursor {
                    return Ok(EventsPage {
                        events: buffer,
                        continuation_token: Some(c.into_rpc_cursor().to_string()),
                    });
                }

                if let Some(executor) = self.pending_executor() {
                    let cursor = utils::events::fetch_pending_events(
                        &executor,
                        &filter,
                        chunk_size,
                        token,
                        &mut buffer,
                    )?;

                    Ok(EventsPage {
                        events: buffer,
                        continuation_token: Some(cursor.into_rpc_cursor().to_string()),
                    })
                } else {
                    let cursor = Cursor::new_block(latest + 1);
                    Ok(EventsPage {
                        events: buffer,
                        continuation_token: Some(cursor.into_rpc_cursor().to_string()),
                    })
                }
            }

            (EventBlockId::Pending, EventBlockId::Pending) => {
                if let Some(executor) = self.pending_executor() {
                    let cursor = utils::events::fetch_pending_events(
                        &executor,
                        &filter,
                        chunk_size,
                        token,
                        &mut buffer,
                    )?;

                    Ok(EventsPage {
                        events: buffer,
                        continuation_token: Some(cursor.into_rpc_cursor().to_string()),
                    })
                } else {
                    let latest = provider.latest_number()?;
                    let cursor = Cursor::new_block(latest);

                    Ok(EventsPage {
                        events: buffer,
                        continuation_token: Some(cursor.into_rpc_cursor().to_string()),
                    })
                }
            }

            (EventBlockId::Pending, EventBlockId::Num(_)) => {
                Err(StarknetApiError::UnexpectedError {
                    reason: "Invalid block range; `from` block must be lower than `to`".to_string(),
                })
            }
        }
    }

    async fn transaction_status(&self, hash: TxHash) -> StarknetApiResult<TransactionStatus> {
        let status = self
            .on_io_blocking_task(move |this| {
                let provider = this.inner.backend.blockchain.provider();
                let status = provider.transaction_status(hash)?;

                if let Some(status) = status {
                    // TODO: this might not work once we allow querying for 'failed' transactions
                    // from the provider
                    let Some(receipt) = provider.receipt_by_hash(hash)? else {
                        return Err(StarknetApiError::UnexpectedError {
                            reason: "Transaction hash exist, but the receipt is missing"
                                .to_string(),
                        });
                    };

                    let exec_status = if receipt.is_reverted() {
                        TransactionExecutionStatus::Reverted
                    } else {
                        TransactionExecutionStatus::Succeeded
                    };

                    let status = match status {
                        FinalityStatus::AcceptedOnL1 => {
                            TransactionStatus::AcceptedOnL1(exec_status)
                        }
                        FinalityStatus::AcceptedOnL2 => {
                            TransactionStatus::AcceptedOnL2(exec_status)
                        }
                    };

                    return Ok(Some(status));
                }

                // seach in the pending block if the transaction is not found
                if let Some(pending_executor) = this.pending_executor() {
                    let pending_executor = pending_executor.read();
                    let pending_txs = pending_executor.transactions();
                    let (_, res) = pending_txs
                        .iter()
                        .find(|(tx, _)| tx.hash == hash)
                        .ok_or(StarknetApiError::TxnHashNotFound)?;

                    // TODO: should impl From<ExecutionResult> for TransactionStatus
                    let status = match res {
                        ExecutionResult::Failed { .. } => TransactionStatus::Rejected,
                        ExecutionResult::Success { receipt, .. } => {
                            if receipt.is_reverted() {
                                TransactionStatus::AcceptedOnL2(
                                    TransactionExecutionStatus::Reverted,
                                )
                            } else {
                                TransactionStatus::AcceptedOnL2(
                                    TransactionExecutionStatus::Succeeded,
                                )
                            }
                        }
                    };

                    Ok(Some(status))
                } else {
                    // Err(StarknetApiError::TxnHashNotFound)
                    Ok(None)
                }
            })
            .await?;

        if let Some(status) = status {
            Ok(status)
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_transaction_status(hash).await?)
        } else {
            Err(StarknetApiError::TxnHashNotFound)
        }
    }

    async fn block_with_txs(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingBlockWithTxs> {
        let block = self
            .on_io_blocking_task(move |this| {
                let provider = this.inner.backend.blockchain.provider();

                if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                    if let Some(executor) = this.pending_executor() {
                        let block_env = executor.read().block_env();
                        let latest_hash = provider.latest_hash().map_err(StarknetApiError::from)?;

                        let l1_gas_prices = block_env.l1_gas_prices.clone();
                        let l1_data_gas_prices = block_env.l1_data_gas_prices.clone();

                        let header = PartialHeader {
                            l1_da_mode: L1DataAvailabilityMode::Calldata,
                            l1_gas_prices,
                            l1_data_gas_prices,
                            number: block_env.number,
                            parent_hash: latest_hash,
                            timestamp: block_env.timestamp,
                            sequencer_address: block_env.sequencer_address,
                            protocol_version: this.inner.backend.chain_spec.version.clone(),
                        };

                        // TODO(kariy): create a method that can perform this filtering for us
                        // instead of doing it manually.

                        // A block should only include successful transactions, we filter out the
                        // failed ones (didn't pass validation stage).
                        let transactions = executor
                            .read()
                            .transactions()
                            .iter()
                            .filter(|(_, receipt)| receipt.is_success())
                            .map(|(tx, _)| tx.clone())
                            .collect::<Vec<_>>();

                        let block = PendingBlockWithTxs::new(header, transactions);
                        return Ok(Some(MaybePendingBlockWithTxs::Pending(block)));
                    }
                }

                if let Some(num) = provider.convert_block_id(block_id)? {
                    let block = katana_rpc_types_builder::BlockBuilder::new(num.into(), provider)
                        .build()?
                        .map(MaybePendingBlockWithTxs::Block);

                    StarknetApiResult::Ok(block)
                } else {
                    StarknetApiResult::Ok(None)
                }
            })
            .await?;

        if let Some(block) = block {
            Ok(block)
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_block_with_txs(block_id).await?)
        } else {
            Err(StarknetApiError::BlockNotFound)
        }
    }

    async fn block_with_receipts(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingBlockWithReceipts> {
        let block = self
            .on_io_blocking_task(move |this| {
                let provider = this.inner.backend.blockchain.provider();

                if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                    if let Some(executor) = this.pending_executor() {
                        let block_env = executor.read().block_env();
                        let latest_hash = provider.latest_hash()?;

                        let l1_gas_prices = block_env.l1_gas_prices.clone();
                        let l1_data_gas_prices = block_env.l1_data_gas_prices.clone();

                        let header = PartialHeader {
                            l1_gas_prices,
                            l1_data_gas_prices,
                            number: block_env.number,
                            parent_hash: latest_hash,
                            timestamp: block_env.timestamp,
                            l1_da_mode: L1DataAvailabilityMode::Calldata,
                            sequencer_address: block_env.sequencer_address,
                            protocol_version: this.inner.backend.chain_spec.version.clone(),
                        };

                        let receipts = executor
                            .read()
                            .transactions()
                            .iter()
                            .filter_map(|(tx, result)| match result {
                                ExecutionResult::Success { receipt, .. } => {
                                    Some((tx.clone(), receipt.clone()))
                                }
                                ExecutionResult::Failed { .. } => None,
                            })
                            .collect::<Vec<_>>();

                        let block = PendingBlockWithReceipts::new(header, receipts.into_iter());
                        return Ok(Some(MaybePendingBlockWithReceipts::Pending(block)));
                    }
                }

                if let Some(num) = provider.convert_block_id(block_id)? {
                    let block = katana_rpc_types_builder::BlockBuilder::new(num.into(), provider)
                        .build_with_receipts()?
                        .map(MaybePendingBlockWithReceipts::Block);

                    StarknetApiResult::Ok(block)
                } else {
                    StarknetApiResult::Ok(None)
                }
            })
            .await?;

        if let Some(block) = block {
            Ok(block)
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_block_with_receipts(block_id).await?)
        } else {
            Err(StarknetApiError::BlockNotFound)
        }
    }

    async fn block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingBlockWithTxHashes> {
        let block = self
            .on_io_blocking_task(move |this| {
                let provider = this.inner.backend.blockchain.provider();

                if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                    if let Some(executor) = this.pending_executor() {
                        let block_env = executor.read().block_env();
                        let latest_hash = provider.latest_hash().map_err(StarknetApiError::from)?;

                        let l1_gas_prices = block_env.l1_gas_prices.clone();
                        let l1_data_gas_prices = block_env.l1_data_gas_prices.clone();

                        let header = PartialHeader {
                            l1_da_mode: L1DataAvailabilityMode::Calldata,
                            l1_data_gas_prices,
                            l1_gas_prices,
                            number: block_env.number,
                            parent_hash: latest_hash,
                            timestamp: block_env.timestamp,
                            protocol_version: this.inner.backend.chain_spec.version.clone(),
                            sequencer_address: block_env.sequencer_address,
                        };

                        // TODO(kariy): create a method that can perform this filtering for us
                        // instead of doing it manually.

                        // A block should only include successful transactions, we filter out the
                        // failed ones (didn't pass validation stage).
                        let transactions = executor
                            .read()
                            .transactions()
                            .iter()
                            .filter(|(_, receipt)| receipt.is_success())
                            .map(|(tx, _)| tx.hash)
                            .collect::<Vec<_>>();

                        let block = PendingBlockWithTxHashes::new(header, transactions);
                        return Ok(Some(MaybePendingBlockWithTxHashes::Pending(block)));
                    }
                }

                if let Some(num) = provider.convert_block_id(block_id)? {
                    let block = katana_rpc_types_builder::BlockBuilder::new(num.into(), provider)
                        .build_with_tx_hash()?
                        .map(MaybePendingBlockWithTxHashes::Block);

                    StarknetApiResult::Ok(block)
                } else {
                    StarknetApiResult::Ok(None)
                }
            })
            .await?;

        if let Some(block) = block {
            Ok(block)
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_block_with_tx_hashes(block_id).await?)
        } else {
            Err(StarknetApiError::BlockNotFound)
        }
    }

    async fn state_update(
        &self,
        block_id: BlockIdOrTag,
    ) -> StarknetApiResult<MaybePendingStateUpdate> {
        let state_update = self
            .on_io_blocking_task(move |this| {
                let provider = this.inner.backend.blockchain.provider();

                let block_id = match block_id {
                    BlockIdOrTag::Number(num) => BlockHashOrNumber::Num(num),
                    BlockIdOrTag::Hash(hash) => BlockHashOrNumber::Hash(hash),

                    BlockIdOrTag::Tag(BlockTag::Latest) => {
                        provider.latest_number().map(BlockHashOrNumber::Num)?
                    }

                    BlockIdOrTag::Tag(BlockTag::Pending) => {
                        return Err(StarknetApiError::BlockNotFound);
                    }
                };

                let state_update =
                    katana_rpc_types_builder::StateUpdateBuilder::new(block_id, provider)
                        .build()?
                        .map(MaybePendingStateUpdate::Update);

                StarknetApiResult::Ok(state_update)
            })
            .await?;

        if let Some(state_update) = state_update {
            Ok(state_update)
        } else if let Some(client) = &self.inner.forked_client {
            Ok(client.get_state_update(block_id).await?)
        } else {
            Err(StarknetApiError::BlockNotFound)
        }
    }
}
