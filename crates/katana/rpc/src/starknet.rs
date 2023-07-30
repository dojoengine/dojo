use std::sync::Arc;

use blockifier::state::errors::StateError;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transactions::DeclareTransaction;
use jsonrpsee::core::{async_trait, Error};
use katana_core::backend::contract::StarknetContract;
use katana_core::backend::storage::transaction::{KnownTransaction, PendingTransaction};
use katana_core::backend::ExternalFunctionCall;
use katana_core::sequencer::Sequencer;
use katana_core::sequencer_error::SequencerError;
use katana_core::utils::contract::{
    legacy_inner_to_rpc_class, legacy_rpc_to_inner_class, rpc_to_inner_class,
};
use katana_core::utils::starkfelt_to_u128;
use katana_core::utils::transaction::compute_deploy_account_v1_transaction_hash;
use katana_core::utils::transaction::{
    compute_declare_v1_transaction_hash, compute_declare_v2_transaction_hash,
    compute_invoke_v1_transaction_hash,
};
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BlockTag, BroadcastedDeclareTransaction,
    BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    ContractClass, DeclareTransactionResult, DeployAccountTransactionResult, EventFilterWithPage,
    EventsPage, FeeEstimate, FieldElement, FunctionCall, InvokeTransactionResult,
    MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs, MaybePendingTransactionReceipt,
    StateUpdate, Transaction,
};
use starknet::core::utils::get_contract_address;
use starknet_api::core::{
    ClassHash, CompiledClassHash, ContractAddress, EntryPointSelector, Nonce, PatriciaKey,
};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeclareTransactionV0V1, DeclareTransactionV2,
    DeployAccountTransaction, Fee, InvokeTransaction, InvokeTransactionV1, TransactionHash,
    TransactionSignature, TransactionVersion,
};
use starknet_api::{patricia_key, stark_felt};

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
    ) -> Result<Transaction, Error> {
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
    ) -> Result<Transaction, Error> {
        let block = self
            .sequencer
            .block(block_id)
            .await
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        let hash: FieldElement = block
            .transactions()
            .get(index)
            .map(|t| t.transaction.transaction_hash().0.into())
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

    async fn pending_transactions(&self) -> Result<Vec<Transaction>, Error> {
        let block = self.sequencer.block(BlockId::Tag(BlockTag::Pending)).await;

        Ok(block
            .map(|b| {
                b.transactions()
                    .iter()
                    .map(|tx| KnownTransaction::Pending(PendingTransaction(tx.clone())).into())
                    .collect::<Vec<Transaction>>()
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

        let BroadcastedDeployAccountTransaction {
            class_hash,
            constructor_calldata,
            contract_address_salt,
            max_fee,
            nonce,
            signature,
        } = deploy_account_transaction;

        let contract_address = get_contract_address(
            contract_address_salt,
            class_hash,
            &constructor_calldata,
            FieldElement::ZERO,
        );

        let transaction_hash = compute_deploy_account_v1_transaction_hash(
            contract_address,
            &constructor_calldata,
            class_hash,
            contract_address_salt,
            max_fee,
            chain_id,
            nonce,
        );

        let transaction = DeployAccountTransaction {
            signature: TransactionSignature(signature.into_iter().map(|s| s.into()).collect()),
            contract_address_salt: ContractAddressSalt(StarkFelt::from(contract_address_salt)),
            constructor_calldata: Calldata(Arc::new(
                constructor_calldata.into_iter().map(|d| d.into()).collect(),
            )),
            class_hash: ClassHash(class_hash.into()),
            contract_address: ContractAddress(patricia_key!(contract_address)),
            max_fee: Fee(starkfelt_to_u128(max_fee.into())
                .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
            nonce: Nonce(nonce.into()),
            transaction_hash: TransactionHash(transaction_hash.into()),
            version: TransactionVersion(stark_felt!(1_u32)),
        };

        self.sequencer.add_deploy_account_transaction(transaction).await;

        Ok(DeployAccountTransactionResult { transaction_hash, contract_address })
    }

    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTransaction>,
        block_id: BlockId,
    ) -> Result<Vec<FeeEstimate>, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.chain_id().await.as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let mut res = Vec::new();

        for r in request {
            let transaction = match r {
                BroadcastedTransaction::Declare(BroadcastedDeclareTransaction::V1(tx)) => {
                    let (class_hash, contract) = legacy_rpc_to_inner_class(&tx.contract_class)?;

                    let transaction_hash = compute_declare_v1_transaction_hash(
                        tx.sender_address,
                        class_hash,
                        tx.max_fee,
                        chain_id,
                        tx.nonce,
                    );

                    let transaction = DeclareTransactionV0V1 {
                        nonce: Nonce(tx.nonce.into()),
                        class_hash: ClassHash(class_hash.into()),
                        transaction_hash: TransactionHash(transaction_hash.into()),
                        sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                        max_fee: Fee(starkfelt_to_u128(tx.max_fee.into())
                            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                        signature: TransactionSignature(
                            tx.signature.into_iter().map(|e| e.into()).collect(),
                        ),
                    };

                    AccountTransaction::Declare(
                        DeclareTransaction::new(
                            starknet_api::transaction::DeclareTransaction::V1(transaction),
                            contract,
                        )
                        .map_err(|_| Error::from(StarknetApiError::InternalServerError))?,
                    )
                }
                BroadcastedTransaction::Declare(BroadcastedDeclareTransaction::V2(tx)) => {
                    let (class_hash, contract_class) = rpc_to_inner_class(&tx.contract_class)
                        .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

                    let transaction_hash = compute_declare_v2_transaction_hash(
                        tx.sender_address,
                        class_hash,
                        tx.max_fee,
                        chain_id,
                        tx.nonce,
                        tx.compiled_class_hash,
                    );

                    let transaction = DeclareTransactionV2 {
                        nonce: Nonce(tx.nonce.into()),
                        class_hash: ClassHash(class_hash.into()),
                        transaction_hash: TransactionHash(transaction_hash.into()),
                        sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                        compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                        max_fee: Fee(starkfelt_to_u128(tx.max_fee.into())
                            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                        signature: TransactionSignature(
                            tx.signature.into_iter().map(|e| e.into()).collect(),
                        ),
                    };

                    AccountTransaction::Declare(
                        DeclareTransaction::new(
                            starknet_api::transaction::DeclareTransaction::V2(transaction),
                            contract_class,
                        )
                        .map_err(|_| Error::from(StarknetApiError::InternalServerError))?,
                    )
                }

                BroadcastedTransaction::Invoke(BroadcastedInvokeTransaction::V1(transaction)) => {
                    let transaction_hash = compute_invoke_v1_transaction_hash(
                        transaction.sender_address,
                        &transaction.calldata,
                        transaction.max_fee,
                        chain_id,
                        transaction.nonce,
                    );

                    let transaction = InvokeTransactionV1 {
                        transaction_hash: TransactionHash(StarkFelt::from(transaction_hash)),
                        sender_address: ContractAddress(patricia_key!(transaction.sender_address)),
                        nonce: Nonce(StarkFelt::from(transaction.nonce)),
                        calldata: Calldata(Arc::new(
                            transaction.calldata.into_iter().map(StarkFelt::from).collect(),
                        )),
                        max_fee: Fee(starkfelt_to_u128(StarkFelt::from(transaction.max_fee))
                            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                        signature: TransactionSignature(
                            transaction.signature.into_iter().map(StarkFelt::from).collect(),
                        ),
                    };

                    AccountTransaction::Invoke(InvokeTransaction::V1(transaction))
                }

                BroadcastedTransaction::DeployAccount(BroadcastedDeployAccountTransaction {
                    max_fee,
                    signature,
                    nonce,
                    contract_address_salt,
                    constructor_calldata,
                    class_hash,
                }) => {
                    let contract_address = get_contract_address(
                        contract_address_salt,
                        class_hash,
                        &constructor_calldata,
                        FieldElement::ZERO,
                    );

                    let transaction_hash = compute_deploy_account_v1_transaction_hash(
                        contract_address,
                        &constructor_calldata,
                        class_hash,
                        contract_address_salt,
                        max_fee,
                        chain_id,
                        nonce,
                    );

                    let transaction = DeployAccountTransaction {
                        signature: TransactionSignature(
                            signature.into_iter().map(|s| s.into()).collect(),
                        ),
                        contract_address_salt: ContractAddressSalt(StarkFelt::from(
                            contract_address_salt,
                        )),
                        constructor_calldata: Calldata(Arc::new(
                            constructor_calldata.into_iter().map(|d| d.into()).collect(),
                        )),
                        class_hash: ClassHash(class_hash.into()),
                        contract_address: ContractAddress(patricia_key!(contract_address)),
                        max_fee: Fee(starkfelt_to_u128(max_fee.into())
                            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                        nonce: Nonce(nonce.into()),
                        transaction_hash: TransactionHash(transaction_hash.into()),
                        version: TransactionVersion(stark_felt!(1_u32)),
                    };

                    AccountTransaction::DeployAccount(transaction)
                }

                _ => return Err(Error::from(StarknetApiError::UnsupportedTransactionVersion)),
            };

            let fee_estimate =
                self.sequencer.estimate_fee(transaction, block_id).await.map_err(|e| match e {
                    SequencerError::BlockNotFound(_) => {
                        Error::from(StarknetApiError::BlockNotFound)
                    }
                    SequencerError::TransactionExecution(_) => {
                        Error::from(StarknetApiError::ContractError)
                    }
                    _ => Error::from(StarknetApiError::InternalServerError),
                })?;

            res.push(FeeEstimate {
                gas_price: fee_estimate.gas_price,
                overall_fee: fee_estimate.overall_fee,
                gas_consumed: fee_estimate.gas_consumed,
            });
        }

        Ok(res)
    }

    async fn add_declare_transaction(
        &self,
        declare_transaction: BroadcastedDeclareTransaction,
    ) -> Result<DeclareTransactionResult, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.chain_id().await.as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;
        let (transaction_hash, class_hash, transaction, sierra_class) = match declare_transaction {
            BroadcastedDeclareTransaction::V1(tx) => {
                let (class_hash, contract) = legacy_rpc_to_inner_class(&tx.contract_class)?;

                let transaction_hash = compute_declare_v1_transaction_hash(
                    tx.sender_address,
                    class_hash,
                    tx.max_fee,
                    chain_id,
                    tx.nonce,
                );

                let transaction = DeclareTransactionV0V1 {
                    transaction_hash: TransactionHash(transaction_hash.into()),
                    class_hash: ClassHash(class_hash.into()),
                    sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                    nonce: Nonce(tx.nonce.into()),
                    max_fee: Fee(starkfelt_to_u128(tx.max_fee.into())
                        .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                    signature: TransactionSignature(
                        tx.signature.into_iter().map(|e| e.into()).collect(),
                    ),
                };

                (
                    transaction_hash,
                    class_hash,
                    DeclareTransaction::new(
                        starknet_api::transaction::DeclareTransaction::V1(transaction),
                        contract,
                    )
                    .map_err(|_| Error::from(StarknetApiError::InternalServerError))?,
                    None,
                )
            }
            BroadcastedDeclareTransaction::V2(tx) => {
                let (class_hash, contract_class) = rpc_to_inner_class(&tx.contract_class)
                    .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

                let transaction_hash = compute_declare_v2_transaction_hash(
                    tx.sender_address,
                    class_hash,
                    tx.max_fee,
                    chain_id,
                    tx.nonce,
                    tx.compiled_class_hash,
                );

                let transaction = DeclareTransactionV2 {
                    nonce: Nonce(tx.nonce.into()),
                    class_hash: ClassHash(class_hash.into()),
                    transaction_hash: TransactionHash(transaction_hash.into()),
                    sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                    compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                    max_fee: Fee(starkfelt_to_u128(tx.max_fee.into())
                        .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                    signature: TransactionSignature(
                        tx.signature.into_iter().map(|e| e.into()).collect(),
                    ),
                };

                (
                    transaction_hash,
                    class_hash,
                    DeclareTransaction::new(
                        starknet_api::transaction::DeclareTransaction::V2(transaction),
                        contract_class,
                    )
                    .map_err(|_| Error::from(StarknetApiError::InternalServerError))?,
                    Some(tx.contract_class.as_ref().clone()),
                )
            }
        };

        self.sequencer.add_declare_transaction(transaction, sierra_class).await;

        Ok(DeclareTransactionResult { transaction_hash, class_hash })
    }

    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTransaction,
    ) -> Result<InvokeTransactionResult, Error> {
        match invoke_transaction {
            BroadcastedInvokeTransaction::V1(transaction) => {
                let chain_id = FieldElement::from_hex_be(&self.sequencer.chain_id().await.as_hex())
                    .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

                let transaction_hash = compute_invoke_v1_transaction_hash(
                    transaction.sender_address,
                    &transaction.calldata,
                    transaction.max_fee,
                    chain_id,
                    transaction.nonce,
                );

                let transaction = InvokeTransactionV1 {
                    transaction_hash: TransactionHash(StarkFelt::from(transaction_hash)),
                    sender_address: ContractAddress(patricia_key!(transaction.sender_address)),
                    nonce: Nonce(StarkFelt::from(transaction.nonce)),
                    calldata: Calldata(Arc::new(
                        transaction.calldata.into_iter().map(StarkFelt::from).collect(),
                    )),
                    max_fee: Fee(starkfelt_to_u128(StarkFelt::from(transaction.max_fee))
                        .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                    signature: TransactionSignature(
                        transaction.signature.into_iter().map(StarkFelt::from).collect(),
                    ),
                };

                self.sequencer.add_invoke_transaction(InvokeTransaction::V1(transaction)).await;

                Ok(InvokeTransactionResult { transaction_hash })
            }

            _ => Err(Error::from(StarknetApiError::UnsupportedTransactionVersion)),
        }
    }
}
