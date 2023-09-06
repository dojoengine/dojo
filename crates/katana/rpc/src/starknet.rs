use std::sync::Arc;

use blockifier::state::errors::StateError;
use jsonrpsee::core::{async_trait, Error};
use katana_core::backend::contract::StarknetContract;
use katana_core::backend::storage::transaction::{
    DeclareTransaction, DeployAccountTransaction, InvokeTransaction, KnownTransaction,
    PendingTransaction, Transaction,
};
use katana_core::backend::ExternalFunctionCall;
use katana_core::sequencer::Sequencer;
use katana_core::sequencer_error::SequencerError;
use katana_core::utils::contract::legacy_inner_to_rpc_class;
use katana_core::utils::transaction::{
    broadcasted_declare_rpc_to_api_transaction, broadcasted_deploy_account_rpc_to_api_transaction,
    broadcasted_invoke_rpc_to_api_transaction,
};
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BlockTag, BroadcastedDeclareTransaction,
    BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    ContractClass, DeclareTransactionResult, DeployAccountTransactionResult, EventFilterWithPage,
    EventsPage, FeeEstimate, FieldElement, FunctionCall, InvokeTransactionResult,
    MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs, MaybePendingTransactionReceipt,
    StateUpdate, Transaction as RpcTransaction,
};
use starknet_api::core::{ClassHash, ContractAddress, EntryPointSelector, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;
use starknet_api::transaction::Calldata;

use crate::api::starknet::{Felt, StarknetApiError, StarknetApiServer};

pub struct StarknetApi<S> {
    sequencer: S,
}

impl<S> StarknetApi<S>
where
    S: Sequencer + Send + Sync + 'static,
{
    pub fn new(sequencer: S) -> Self {
        Self { sequencer }
    }
}
#[async_trait]
impl<S> StarknetApiServer for StarknetApi<S>
where
    S: Sequencer + Send + Sync + 'static,
{
    async fn chain_id(&self) -> Result<String, Error> {
        Ok(self.sequencer.chain_id().await.as_hex())
    }

    async fn nonce(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<Felt, Error> {
        let nonce = self
            .sequencer
            .nonce_at(block_id, ContractAddress(patricia_key!(contract_address)))
            .await
            .map_err(|e| match e {
                SequencerError::StateNotFound(_) => Error::from(StarknetApiError::BlockNotFound),
                SequencerError::ContractNotFound(_) => {
                    Error::from(StarknetApiError::ContractNotFound)
                }
                _ => Error::from(StarknetApiError::InternalServerError),
            })?;

        Ok(Felt(nonce.0.into()))
    }

    async fn block_number(&self) -> Result<u64, Error> {
        Ok(self.sequencer.block_number().await)
    }

    async fn transaction_by_hash(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<RpcTransaction, Error> {
        let transaction = self
            .sequencer
            .transaction(&transaction_hash)
            .await
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;

        Ok(transaction.into())
    }

    async fn block_transaction_count(&self, block_id: BlockId) -> Result<u64, Error> {
        let block = self
            .sequencer
            .block(block_id)
            .await
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        Ok(block.transaction_count() as u64)
    }

    async fn class_at(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<ContractClass, Error> {
        let class_hash = self.class_hash_at(block_id, contract_address).await?;
        self.class(block_id, class_hash.0).await
    }

    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error> {
        let (block_hash, block_number) = self.sequencer.block_hash_and_number().await;
        Ok(BlockHashAndNumber { block_hash, block_number })
    }

    async fn block_with_tx_hashes(
        &self,
        block_id: BlockId,
    ) -> Result<MaybePendingBlockWithTxHashes, Error> {
        let block = self
            .sequencer
            .block(block_id)
            .await
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        Ok(block.into())
    }

    async fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockId,
        index: usize,
    ) -> Result<RpcTransaction, Error> {
        let block = self
            .sequencer
            .block(block_id)
            .await
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        let hash: FieldElement = block
            .transactions()
            .get(index)
            .map(|t| t.inner.hash())
            .ok_or(Error::from(StarknetApiError::InvalidTxnIndex))?;

        self.transaction_by_hash(hash).await
    }

    async fn block_with_txs(&self, block_id: BlockId) -> Result<MaybePendingBlockWithTxs, Error> {
        let block = self
            .sequencer
            .block(block_id)
            .await
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        Ok(block.into())
    }

    async fn state_update(&self, block_id: BlockId) -> Result<StateUpdate, Error> {
        self.sequencer
            .state_update(block_id)
            .await
            .map_err(|_| Error::from(StarknetApiError::BlockNotFound))
    }

    async fn transaction_receipt(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<MaybePendingTransactionReceipt, Error> {
        self.sequencer
            .transaction_receipt(&transaction_hash)
            .await
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))
    }

    async fn class_hash_at(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<Felt, Error> {
        let class_hash = self
            .sequencer
            .class_hash_at(block_id, ContractAddress(patricia_key!(contract_address)))
            .await
            .map_err(|e| match e {
                SequencerError::BlockNotFound(_) => StarknetApiError::BlockNotFound,
                SequencerError::ContractNotFound(_) => StarknetApiError::ContractNotFound,
                _ => StarknetApiError::InternalServerError,
            })?;

        Ok(Felt(class_hash.0.into()))
    }

    async fn class(
        &self,
        block_id: BlockId,
        class_hash: FieldElement,
    ) -> Result<ContractClass, Error> {
        let contract = self.sequencer.class(block_id, ClassHash(class_hash.into())).await.map_err(
            |e| match e {
                SequencerError::BlockNotFound(_) => StarknetApiError::BlockNotFound,
                SequencerError::State(StateError::UndeclaredClassHash(_)) => {
                    StarknetApiError::ClassHashNotFound
                }
                _ => StarknetApiError::InternalServerError,
            },
        )?;

        match contract {
            StarknetContract::Legacy(c) => {
                let contract = legacy_inner_to_rpc_class(c)
                    .map_err(|_| StarknetApiError::InternalServerError)?;
                Ok(contract)
            }
            StarknetContract::Sierra(c) => Ok(ContractClass::Sierra(c)),
        }
    }

    async fn events(&self, filter: EventFilterWithPage) -> Result<EventsPage, Error> {
        let from_block = filter.event_filter.from_block.unwrap_or(BlockId::Number(0));
        let to_block = filter.event_filter.to_block.unwrap_or(BlockId::Tag(BlockTag::Latest));

        let keys = filter.event_filter.keys;
        let keys = keys.filter(|keys| !(keys.len() == 1 && keys.is_empty()));

        let events = self
            .sequencer
            .events(
                from_block,
                to_block,
                filter.event_filter.address,
                keys,
                filter.result_page_request.continuation_token,
                filter.result_page_request.chunk_size,
            )
            .await
            .map_err(|e| match e {
                SequencerError::BlockNotFound(_) => StarknetApiError::BlockNotFound,
                _ => StarknetApiError::InternalServerError,
            })?;

        Ok(events)
    }

    async fn pending_transactions(&self) -> Result<Vec<RpcTransaction>, Error> {
        let block = self.sequencer.block(BlockId::Tag(BlockTag::Pending)).await;

        Ok(block
            .map(|b| {
                b.transactions()
                    .iter()
                    .map(|tx| KnownTransaction::Pending(PendingTransaction(tx.clone())).into())
                    .collect::<Vec<RpcTransaction>>()
            })
            .unwrap_or(Vec::new()))
    }

    async fn call(&self, request: FunctionCall, block_id: BlockId) -> Result<Vec<Felt>, Error> {
        let call = ExternalFunctionCall {
            contract_address: ContractAddress(patricia_key!(request.contract_address)),
            calldata: Calldata(Arc::new(
                request.calldata.into_iter().map(StarkFelt::from).collect(),
            )),
            entry_point_selector: EntryPointSelector(StarkFelt::from(request.entry_point_selector)),
        };

        let res = self.sequencer.call(block_id, call).await.map_err(|e| match e {
            SequencerError::BlockNotFound(_) => Error::from(StarknetApiError::BlockNotFound),
            SequencerError::ContractNotFound(_) => Error::from(StarknetApiError::ContractNotFound),
            SequencerError::EntryPointExecution(_) => Error::from(StarknetApiError::ContractError),
            _ => Error::from(StarknetApiError::InternalServerError),
        })?;

        let mut values = vec![];

        for f in res.into_iter() {
            values.push(Felt(f.into()));
        }

        Ok(values)
    }

    async fn storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        block_id: BlockId,
    ) -> Result<Felt, Error> {
        let value = self
            .sequencer
            .storage_at(
                ContractAddress(patricia_key!(contract_address)),
                StorageKey(patricia_key!(key)),
                block_id,
            )
            .await
            .map_err(|e| match e {
                SequencerError::StateNotFound(_) => Error::from(StarknetApiError::BlockNotFound),
                SequencerError::State(_) => Error::from(StarknetApiError::ContractNotFound),
                _ => Error::from(StarknetApiError::InternalServerError),
            })?;

        Ok(Felt(value.into()))
    }

    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTransaction,
    ) -> Result<DeployAccountTransactionResult, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.chain_id().await.as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let (transaction, contract_address) =
            broadcasted_deploy_account_rpc_to_api_transaction(deploy_account_transaction, chain_id);
        let transaction_hash = transaction.transaction_hash.0.into();

        self.sequencer
            .add_deploy_account_transaction(DeployAccountTransaction {
                contract_address,
                inner: transaction,
            })
            .await;

        Ok(DeployAccountTransactionResult { transaction_hash, contract_address })
    }

    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTransaction>,
        block_id: BlockId,
    ) -> Result<Vec<FeeEstimate>, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.chain_id().await.as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let transactions = request
            .into_iter()
            .map(|r| match r {
                BroadcastedTransaction::Declare(tx) => {
                    let sierra_class = match tx {
                        BroadcastedDeclareTransaction::V2(ref tx) => {
                            Some(tx.contract_class.as_ref().clone())
                        }
                        _ => None,
                    };

                    let (transaction, compiled_class) =
                        broadcasted_declare_rpc_to_api_transaction(tx, chain_id).unwrap();

                    Transaction::Declare(DeclareTransaction {
                        sierra_class,
                        compiled_class,
                        inner: transaction,
                    })
                }

                BroadcastedTransaction::Invoke(tx) => {
                    let transaction = broadcasted_invoke_rpc_to_api_transaction(tx, chain_id);
                    Transaction::Invoke(InvokeTransaction(transaction))
                }

                BroadcastedTransaction::DeployAccount(tx) => {
                    let (transaction, contract_address) =
                        broadcasted_deploy_account_rpc_to_api_transaction(tx, chain_id);

                    Transaction::DeployAccount(DeployAccountTransaction {
                        contract_address,
                        inner: transaction,
                    })
                }
            })
            .collect::<Vec<_>>();

        let res =
            self.sequencer.estimate_fee(transactions, block_id).await.map_err(|e| match e {
                SequencerError::BlockNotFound(_) => Error::from(StarknetApiError::BlockNotFound),
                SequencerError::TransactionExecution(_) => {
                    Error::from(StarknetApiError::ContractError)
                }
                _ => Error::from(StarknetApiError::InternalServerError),
            })?;

        Ok(res)
    }

    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTransaction,
    ) -> Result<DeclareTransactionResult, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.chain_id().await.as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let sierra_class = match declare_transaction {
            BroadcastedDeclareTransaction::V2(ref tx) => Some(tx.contract_class.as_ref().clone()),
            _ => None,
        };

        let (transaction, contract_class) =
            broadcasted_declare_rpc_to_api_transaction(declare_transaction, chain_id).unwrap();

        let transaction_hash = transaction.transaction_hash().0.into();
        let class_hash = transaction.class_hash().0.into();

        self.sequencer.add_declare_transaction(DeclareTransaction {
            sierra_class,
            inner: transaction,
            compiled_class: contract_class,
        });

        Ok(DeclareTransactionResult { transaction_hash, class_hash })
    }

    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTransaction,
    ) -> Result<InvokeTransactionResult, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.chain_id().await.as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let transaction = broadcasted_invoke_rpc_to_api_transaction(invoke_transaction, chain_id);
        let transaction_hash = transaction.transaction_hash().0.into();

        self.sequencer.add_invoke_transaction(InvokeTransaction(transaction));

        Ok(InvokeTransactionResult { transaction_hash })
    }
}
