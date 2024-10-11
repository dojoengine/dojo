use jsonrpsee::core::{async_trait, Error, RpcResult};
use katana_executor::{EntryPointCall, ExecutionResult, ExecutorFactory};
use katana_primitives::block::{BlockHashOrNumber, BlockIdOrTag, FinalityStatus, PartialHeader};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxHash};
use katana_primitives::version::CURRENT_STARKNET_VERSION;
use katana_primitives::Felt;
use katana_provider::traits::block::{BlockHashProvider, BlockIdReader, BlockNumberProvider};
use katana_provider::traits::transaction::TransactionProvider;
use katana_rpc_api::starknet::StarknetApiServer;
use katana_rpc_types::block::{
    BlockHashAndNumber, MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes,
    MaybePendingBlockWithTxs, PendingBlockWithReceipts, PendingBlockWithTxHashes,
    PendingBlockWithTxs,
};
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::event::{EventFilterWithPage, EventsPage};
use katana_rpc_types::message::MsgFromL1;
use katana_rpc_types::receipt::{ReceiptBlock, TxReceiptWithBlockInfo};
use katana_rpc_types::state_update::StateUpdate;
use katana_rpc_types::transaction::{BroadcastedTx, Tx};
use katana_rpc_types::{
    ContractClass, FeeEstimate, FeltAsHex, FunctionCall, SimulationFlagForEstimateFee,
};
use katana_rpc_types_builder::ReceiptBuilder;
use starknet::core::types::{BlockTag, TransactionStatus};

use super::StarknetApi;

#[async_trait]
impl<EF: ExecutorFactory> StarknetApiServer for StarknetApi<EF> {
    async fn chain_id(&self) -> RpcResult<FeltAsHex> {
        Ok(self.inner.backend.chain_spec.id.id().into())
    }

    async fn get_nonce(
        &self,
        block_id: BlockIdOrTag,
        contract_address: Felt,
    ) -> RpcResult<FeltAsHex> {
        Ok(self.nonce_at(block_id, contract_address.into()).await?.into())
    }

    async fn block_number(&self) -> RpcResult<u64> {
        Ok(self.latest_block_number().await?)
    }

    async fn get_transaction_by_hash(&self, transaction_hash: Felt) -> RpcResult<Tx> {
        Ok(self.transaction(transaction_hash).await?.into())
    }

    async fn get_block_transaction_count(&self, block_id: BlockIdOrTag) -> RpcResult<u64> {
        self.on_io_blocking_task(move |this| Ok(this.block_tx_count(block_id)?)).await
    }

    async fn get_class_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: Felt,
    ) -> RpcResult<ContractClass> {
        Ok(self.class_at_address(block_id, contract_address.into()).await?)
    }

    async fn block_hash_and_number(&self) -> RpcResult<BlockHashAndNumber> {
        self.on_io_blocking_task(move |this| {
            let res = this.block_hash_and_number()?;
            Ok(res.into())
        })
        .await
    }

    async fn get_block_with_tx_hashes(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithTxHashes> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.backend.blockchain.provider();

            if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                if let Some(executor) = this.pending_executor() {
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

    async fn get_transaction_by_block_id_and_index(
        &self,
        block_id: BlockIdOrTag,
        index: u64,
    ) -> RpcResult<Tx> {
        self.on_io_blocking_task(move |this| {
            // TEMP: have to handle pending tag independently for now
            let tx = if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                let Some(executor) = this.pending_executor() else {
                    return Err(StarknetApiError::BlockNotFound.into());
                };

                let executor = executor.read();
                let pending_txs = executor.transactions();
                pending_txs.get(index as usize).map(|(tx, _)| tx.clone())
            } else {
                let provider = &this.inner.backend.blockchain.provider();

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

    async fn get_block_with_txs(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithTxs> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.backend.blockchain.provider();

            if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                if let Some(executor) = this.pending_executor() {
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

    async fn get_block_with_receipts(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<MaybePendingBlockWithReceipts> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.backend.blockchain.provider();

            if BlockIdOrTag::Tag(BlockTag::Pending) == block_id {
                if let Some(executor) = this.pending_executor() {
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

                    return Ok(MaybePendingBlockWithReceipts::Pending(
                        PendingBlockWithReceipts::new(header, receipts.into_iter()),
                    ));
                }
            }

            let block_num = BlockIdReader::convert_block_id(provider, block_id)
                .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?
                .map(BlockHashOrNumber::Num)
                .ok_or(StarknetApiError::BlockNotFound)?;

            let block = katana_rpc_types_builder::BlockBuilder::new(block_num, provider)
                .build_with_receipts()
                .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?
                .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

            Ok(MaybePendingBlockWithReceipts::Block(block))
        })
        .await
    }

    async fn get_state_update(&self, block_id: BlockIdOrTag) -> RpcResult<StateUpdate> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.backend.blockchain.provider();

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

    async fn get_transaction_receipt(
        &self,
        transaction_hash: Felt,
    ) -> RpcResult<TxReceiptWithBlockInfo> {
        self.on_io_blocking_task(move |this| {
            let provider = this.inner.backend.blockchain.provider();
            let receipt = ReceiptBuilder::new(transaction_hash, provider)
                .build()
                .map_err(|e| StarknetApiError::UnexpectedError { reason: e.to_string() })?;

            match receipt {
                Some(receipt) => Ok(receipt),

                None => {
                    let executor = this.pending_executor();
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

                    Ok(TxReceiptWithBlockInfo::new(
                        ReceiptBlock::Pending,
                        transaction_hash,
                        FinalityStatus::AcceptedOnL2,
                        pending_receipt,
                    ))
                }
            }
        })
        .await
    }

    async fn get_class_hash_at(
        &self,
        block_id: BlockIdOrTag,
        contract_address: Felt,
    ) -> RpcResult<FeltAsHex> {
        Ok(self.class_hash_at_address(block_id, contract_address.into()).await?.into())
    }

    async fn get_class(
        &self,
        block_id: BlockIdOrTag,
        class_hash: Felt,
    ) -> RpcResult<ContractClass> {
        Ok(self.class_at_hash(block_id, class_hash).await?)
    }

    async fn get_events(&self, filter: EventFilterWithPage) -> RpcResult<EventsPage> {
        self.on_io_blocking_task(move |this| {
            let EventFilterWithPage { event_filter, result_page_request } = filter;

            let from = match event_filter.from_block {
                Some(id) => id,
                None => BlockIdOrTag::Number(0),
            };

            let to = match event_filter.to_block {
                Some(id) => id,
                None => BlockIdOrTag::Tag(BlockTag::Pending),
            };

            let keys = event_filter.keys.filter(|keys| !(keys.len() == 1 && keys.is_empty()));

            let events = this.events(
                from,
                to,
                event_filter.address.map(|f| f.into()),
                keys,
                result_page_request.continuation_token,
                result_page_request.chunk_size,
            )?;

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

            // get the state and block env at the specified block for function call execution
            let state = this.state(&block_id)?;
            let env = this.block_env_at(&block_id)?;
            let executor = this.inner.backend.executor_factory.with_state_and_block_env(state, env);

            match executor.call(request) {
                Ok(retdata) => Ok(retdata.into_iter().map(|v| v.into()).collect()),
                Err(err) => Err(Error::from(StarknetApiError::ContractError {
                    revert_error: err.to_string(),
                })),
            }
        })
        .await
    }

    async fn get_storage_at(
        &self,
        contract_address: Felt,
        key: Felt,
        block_id: BlockIdOrTag,
    ) -> RpcResult<FeltAsHex> {
        self.on_io_blocking_task(move |this| {
            let value = this.storage_at(contract_address.into(), key, block_id)?;
            Ok(value.into())
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
            let chain_id = this.inner.backend.chain_spec.id;

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
            let should_validate = !(skip_validate
                || this.inner.backend.executor_factory.execution_flags().skip_validate);
            let flags = katana_executor::SimulationFlag {
                skip_validate: !should_validate,
                // We don't care about the nonce when estimating the fee as the nonce value
                // doesn't affect transaction execution.
                //
                // This doesn't completely disregard the nonce as nonce < account nonce will
                // return an error. It only 'relaxes' the check for nonce >= account nonce.
                skip_nonce_check: true,
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
            let chain_id = this.inner.backend.chain_spec.id;

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

    async fn get_transaction_status(
        &self,
        transaction_hash: TxHash,
    ) -> RpcResult<TransactionStatus> {
        Ok(self.transaction_status(transaction_hash).await?)
    }
}
