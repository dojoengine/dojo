use std::str::FromStr;
use std::sync::Arc;

use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transactions::DeclareTransaction;
use jsonrpsee::core::{async_trait, Error};
use jsonrpsee::types::error::CallError;
use katana_core::constants::SEQUENCER_ADDRESS;
use katana_core::sequencer::Sequencer;
use katana_core::sequencer_error::SequencerError;
use katana_core::starknet::transaction::ExternalFunctionCall;
use katana_core::util::{blockifier_contract_class_from_flattened_sierra_class, starkfelt_to_u128};
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BlockStatus, BlockTag, BlockWithTxHashes, BlockWithTxs,
    BroadcastedDeclareTransaction, BroadcastedDeployAccountTransaction,
    BroadcastedInvokeTransaction, BroadcastedTransaction, ContractClass, DeclareTransactionReceipt,
    DeclareTransactionResult, DeployAccountTransactionReceipt, DeployAccountTransactionResult,
    EmittedEvent, Event, EventFilter, EventsPage, FeeEstimate, FieldElement, FlattenedSierraClass,
    FunctionCall, InvokeTransactionReceipt, InvokeTransactionResult, MaybePendingBlockWithTxHashes,
    MaybePendingBlockWithTxs, MaybePendingTransactionReceipt, MsgToL1, PendingBlockWithTxHashes,
    PendingBlockWithTxs, PendingDeclareTransactionReceipt, PendingDeployAccountTransactionReceipt,
    PendingInvokeTransactionReceipt, PendingTransactionReceipt, StateUpdate, Transaction,
    TransactionReceipt, TransactionStatus,
};
use starknet_api::core::{
    ClassHash, CompiledClassHash, ContractAddress, EntryPointSelector, Nonce, PatriciaKey,
};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeclareTransactionV2, Fee, InvokeTransaction,
    InvokeTransactionV1, Transaction as InnerTransaction, TransactionHash, TransactionOutput,
    TransactionSignature,
};
use tokio::sync::RwLock;
use utils::transaction::{
    compute_declare_v2_transaction_hash, compute_invoke_v1_transaction_hash,
    convert_inner_to_rpc_tx,
};

use self::api::{StarknetApiError, StarknetApiServer};
use crate::utils;

pub mod api;

pub struct StarknetRpc<S> {
    sequencer: Arc<RwLock<S>>,
}

impl<S: Sequencer + Send + Sync + 'static> StarknetRpc<S> {
    pub fn new(sequencer: Arc<RwLock<S>>) -> Self {
        Self { sequencer }
    }
}
#[allow(unused)]
#[async_trait]
impl<S: Sequencer + Send + Sync + 'static> StarknetApiServer for StarknetRpc<S> {
    async fn chain_id(&self) -> Result<String, Error> {
        Ok(self.sequencer.read().await.chain_id().as_hex())
    }

    async fn nonce(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<FieldElement, Error> {
        let nonce = self
            .sequencer
            .write()
            .await
            .nonce_at(block_id, ContractAddress(patricia_key!(contract_address)))
            .map_err(|e| match e {
                SequencerError::StateNotFound(_) => Error::from(StarknetApiError::BlockNotFound),
                SequencerError::ContractNotFound(_) => {
                    Error::from(StarknetApiError::ContractNotFound)
                }
                _ => Error::from(StarknetApiError::InternalServerError),
            })?;

        Ok(nonce.0.into())
    }

    async fn block_number(&self) -> Result<u64, Error> {
        Ok(self.sequencer.read().await.block_number().0)
    }

    async fn transaction_by_hash(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<Transaction, Error> {
        let tx = self
            .sequencer
            .write()
            .await
            .transaction(&TransactionHash(StarkFelt::from(transaction_hash)))
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;

        convert_inner_to_rpc_tx(tx).map_err(|_| Error::from(StarknetApiError::InternalServerError))
    }

    async fn block_transaction_count(&self, block_id: BlockId) -> Result<u64, Error> {
        let block = self
            .sequencer
            .read()
            .await
            .block(block_id)
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        block
            .transactions()
            .len()
            .try_into()
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))
    }

    async fn class_at(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<ContractClass, Error> {
        Err(Error::from(StarknetApiError::InternalServerError))
    }

    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error> {
        let (hash, number) = self
            .sequencer
            .read()
            .await
            .block_hash_and_number()
            .ok_or(Error::from(StarknetApiError::NoBlocks))?;

        Ok(BlockHashAndNumber { block_number: number.0, block_hash: hash.0.into() })
    }

    async fn block_with_tx_hashes(
        &self,
        block_id: BlockId,
    ) -> Result<MaybePendingBlockWithTxHashes, Error> {
        let block = self
            .sequencer
            .read()
            .await
            .block(block_id)
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        let sequencer_address = FieldElement::from(*SEQUENCER_ADDRESS);
        let transactions = block
            .transactions()
            .iter()
            .map(|tx| tx.transaction_hash().0.into())
            .collect::<Vec<_>>();

        let timestamp = block.header().timestamp.0;
        let parent_hash = block.header().parent_hash.0.into();

        if BlockId::Tag(BlockTag::Pending) == block_id {
            return Ok(MaybePendingBlockWithTxHashes::PendingBlock(PendingBlockWithTxHashes {
                transactions,
                sequencer_address,
                timestamp,
                parent_hash,
            }));
        }

        Ok(MaybePendingBlockWithTxHashes::Block(BlockWithTxHashes {
            new_root: block.header().state_root.0.into(),
            block_hash: block.header().block_hash.0.into(),
            block_number: block.header().block_number.0,
            status: BlockStatus::AcceptedOnL2,
            transactions,
            sequencer_address,
            timestamp,
            parent_hash,
        }))
    }

    async fn transaction_by_block_id_and_index(
        &self,
        block_id: BlockId,
        index: usize,
    ) -> Result<Transaction, Error> {
        let block = self
            .sequencer
            .read()
            .await
            .block(block_id)
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        let transaction = block
            .transactions()
            .get(index)
            .ok_or(Error::from(StarknetApiError::InvalidTxnIndex))?;

        convert_inner_to_rpc_tx(transaction.clone())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))
    }

    async fn block_with_txs(&self, block_id: BlockId) -> Result<MaybePendingBlockWithTxs, Error> {
        let block = self
            .sequencer
            .read()
            .await
            .block(block_id)
            .ok_or(Error::from(StarknetApiError::BlockNotFound))?;

        let sequencer_address = FieldElement::from(*SEQUENCER_ADDRESS);
        let transactions = block
            .transactions()
            .iter()
            .map(|tx| convert_inner_to_rpc_tx(tx.clone()).unwrap())
            .collect::<Vec<_>>();
        let timestamp = block.header().timestamp.0;
        let parent_hash = block.header().parent_hash.0.into();

        if BlockId::Tag(BlockTag::Pending) == block_id {
            return Ok(MaybePendingBlockWithTxs::PendingBlock(PendingBlockWithTxs {
                transactions,
                sequencer_address,
                timestamp,
                parent_hash,
            }));
        }

        Ok(MaybePendingBlockWithTxs::Block(BlockWithTxs {
            new_root: block.header().state_root.0.into(),
            block_hash: block.block_hash().0.into(),
            block_number: block.block_number().0,
            status: BlockStatus::AcceptedOnL2,
            transactions,
            sequencer_address,
            timestamp,
            parent_hash,
        }))
    }

    async fn state_update(&self, block_id: BlockId) -> Result<StateUpdate, Error> {
        self.sequencer
            .read()
            .await
            .state_update(block_id)
            .map_err(|_| Error::from(StarknetApiError::BlockNotFound))
    }

    async fn transaction_receipt(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<MaybePendingTransactionReceipt, Error> {
        let sequencer = self.sequencer.read().await;
        let hash = TransactionHash(StarkFelt::from(transaction_hash));

        let tx =
            sequencer.transaction(&hash).ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;
        let receipt = sequencer
            .transaction_receipt(&hash)
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;
        let status = sequencer
            .transaction_status(&hash)
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;

        let receipt = match status {
            TransactionStatus::Pending => match receipt.output {
                TransactionOutput::Invoke(output) => {
                    MaybePendingTransactionReceipt::PendingReceipt(
                        PendingTransactionReceipt::Invoke(PendingInvokeTransactionReceipt {
                            transaction_hash,
                            actual_fee: FieldElement::from_str(&format!("{}", output.actual_fee.0))
                                .unwrap(),
                            messages_sent: output
                                .messages_sent
                                .iter()
                                .map(|m| MsgToL1 {
                                    from_address: (*m.from_address.0.key()).into(),
                                    to_address: FieldElement::from_byte_slice_be(
                                        m.to_address.0.as_bytes(),
                                    )
                                    .unwrap(),
                                    payload: m.payload.0.iter().map(|f| (*f).into()).collect(),
                                })
                                .collect(),
                            events: output
                                .events
                                .into_iter()
                                .map(|e| Event {
                                    from_address: (*e.from_address.0.key()).into(),
                                    keys: e.content.keys.into_iter().map(|k| k.0.into()).collect(),
                                    data: e.content.data.0.into_iter().map(|d| d.into()).collect(),
                                })
                                .collect(),
                        }),
                    )
                }

                TransactionOutput::Declare(output) => {
                    MaybePendingTransactionReceipt::PendingReceipt(
                        PendingTransactionReceipt::Declare(PendingDeclareTransactionReceipt {
                            transaction_hash,
                            actual_fee: FieldElement::from_str(&format!("{}", output.actual_fee.0))
                                .unwrap(),
                            messages_sent: output
                                .messages_sent
                                .iter()
                                .map(|m| MsgToL1 {
                                    from_address: (*m.from_address.0.key()).into(),
                                    to_address: FieldElement::from_byte_slice_be(
                                        m.to_address.0.as_bytes(),
                                    )
                                    .unwrap(),
                                    payload: m.payload.0.iter().map(|f| (*f).into()).collect(),
                                })
                                .collect(),
                            events: output
                                .events
                                .into_iter()
                                .map(|e| Event {
                                    from_address: (*e.from_address.0.key()).into(),
                                    keys: e.content.keys.into_iter().map(|k| k.0.into()).collect(),
                                    data: e.content.data.0.into_iter().map(|d| d.into()).collect(),
                                })
                                .collect(),
                        }),
                    )
                }

                TransactionOutput::DeployAccount(output) => {
                    MaybePendingTransactionReceipt::PendingReceipt(
                        PendingTransactionReceipt::DeployAccount(
                            PendingDeployAccountTransactionReceipt {
                                transaction_hash,
                                actual_fee: FieldElement::from_str(&format!(
                                    "{}",
                                    output.actual_fee.0
                                ))
                                .unwrap(),
                                messages_sent: output
                                    .messages_sent
                                    .iter()
                                    .map(|m| MsgToL1 {
                                        from_address: (*m.from_address.0.key()).into(),
                                        to_address: FieldElement::from_byte_slice_be(
                                            m.to_address.0.as_bytes(),
                                        )
                                        .unwrap(),
                                        payload: m.payload.0.iter().map(|f| (*f).into()).collect(),
                                    })
                                    .collect(),
                                events: output
                                    .events
                                    .into_iter()
                                    .map(|e| Event {
                                        from_address: (*e.from_address.0.key()).into(),
                                        keys: e
                                            .content
                                            .keys
                                            .into_iter()
                                            .map(|k| k.0.into())
                                            .collect(),
                                        data: e
                                            .content
                                            .data
                                            .0
                                            .into_iter()
                                            .map(|d| d.into())
                                            .collect(),
                                    })
                                    .collect(),
                            },
                        ),
                    )
                }

                _ => return Err(Error::from(StarknetApiError::UnsupportedTransactionVersion)),
            },

            TransactionStatus::AcceptedOnL2 => match receipt.output {
                TransactionOutput::Invoke(output) => MaybePendingTransactionReceipt::Receipt(
                    TransactionReceipt::Invoke(InvokeTransactionReceipt {
                        transaction_hash,
                        actual_fee: FieldElement::from_str(&format!("{}", output.actual_fee.0))
                            .unwrap(),
                        messages_sent: output
                            .messages_sent
                            .iter()
                            .map(|m| MsgToL1 {
                                from_address: (*m.from_address.0.key()).into(),
                                to_address: FieldElement::from_byte_slice_be(
                                    m.to_address.0.as_bytes(),
                                )
                                .unwrap(),
                                payload: m.payload.0.iter().map(|f| (*f).into()).collect(),
                            })
                            .collect(),
                        events: output
                            .events
                            .into_iter()
                            .map(|e| Event {
                                from_address: (*e.from_address.0.key()).into(),
                                keys: e.content.keys.into_iter().map(|k| k.0.into()).collect(),
                                data: e.content.data.0.into_iter().map(|d| d.into()).collect(),
                            })
                            .collect(),
                        block_hash: receipt.block_hash.0.into(),
                        block_number: receipt.block_number.0,
                        status: TransactionStatus::AcceptedOnL2,
                    }),
                ),

                TransactionOutput::Declare(output) => MaybePendingTransactionReceipt::Receipt(
                    TransactionReceipt::Declare(DeclareTransactionReceipt {
                        transaction_hash,
                        actual_fee: FieldElement::from_str(&format!("{}", output.actual_fee.0))
                            .unwrap(),
                        messages_sent: output
                            .messages_sent
                            .iter()
                            .map(|m| MsgToL1 {
                                from_address: (*m.from_address.0.key()).into(),
                                to_address: FieldElement::from_byte_slice_be(
                                    m.to_address.0.as_bytes(),
                                )
                                .unwrap(),
                                payload: m.payload.0.iter().map(|f| (*f).into()).collect(),
                            })
                            .collect(),
                        events: output
                            .events
                            .into_iter()
                            .map(|e| Event {
                                from_address: (*e.from_address.0.key()).into(),
                                keys: e.content.keys.into_iter().map(|k| k.0.into()).collect(),
                                data: e.content.data.0.into_iter().map(|d| d.into()).collect(),
                            })
                            .collect(),
                        block_hash: receipt.block_hash.0.into(),
                        block_number: receipt.block_number.0,
                        status: TransactionStatus::AcceptedOnL2,
                    }),
                ),

                TransactionOutput::DeployAccount(output) => {
                    MaybePendingTransactionReceipt::Receipt(TransactionReceipt::DeployAccount(
                        DeployAccountTransactionReceipt {
                            transaction_hash,
                            actual_fee: FieldElement::from_str(&format!("{}", output.actual_fee.0))
                                .unwrap(),
                            messages_sent: output
                                .messages_sent
                                .iter()
                                .map(|m| MsgToL1 {
                                    from_address: (*m.from_address.0.key()).into(),
                                    to_address: FieldElement::from_byte_slice_be(
                                        m.to_address.0.as_bytes(),
                                    )
                                    .unwrap(),
                                    payload: m.payload.0.iter().map(|f| (*f).into()).collect(),
                                })
                                .collect(),
                            events: output
                                .events
                                .into_iter()
                                .map(|e| Event {
                                    from_address: (*e.from_address.0.key()).into(),
                                    keys: e.content.keys.into_iter().map(|k| k.0.into()).collect(),
                                    data: e.content.data.0.into_iter().map(|d| d.into()).collect(),
                                })
                                .collect(),
                            block_hash: receipt.block_hash.0.into(),
                            block_number: receipt.block_number.0,
                            status: TransactionStatus::AcceptedOnL2,
                            contract_address: match tx {
                                InnerTransaction::DeployAccount(tx) => {
                                    (*tx.contract_address.0.key()).into()
                                }
                                _ => {
                                    return Err(Error::from(StarknetApiError::InternalServerError));
                                }
                            },
                        },
                    ))
                }

                _ => return Err(Error::from(StarknetApiError::UnsupportedTransactionVersion)),
            },

            _ => return Err(Error::from(StarknetApiError::UnsupportedTransactionVersion)),
        };

        Ok(receipt)
    }

    async fn class_hash_at(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<FieldElement, Error> {
        let class_hash = self
            .sequencer
            .write()
            .await
            .class_hash_at(block_id, ContractAddress(patricia_key!(contract_address)))
            .map_err(|e| match e {
                SequencerError::BlockNotFound(_) => StarknetApiError::BlockNotFound,
                SequencerError::ContractNotFound(_) => StarknetApiError::ContractNotFound,
                _ => StarknetApiError::InternalServerError,
            })?;

        Ok(class_hash.0.into())
    }

    async fn class(
        &self,
        block_id: BlockId,
        class_hash: FieldElement,
    ) -> Result<ContractClass, Error> {
        Err(Error::from(StarknetApiError::InternalServerError))
    }

    async fn events(
        &self,
        filter: EventFilter,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> Result<EventsPage, Error> {
        let from_block = filter.from_block.unwrap_or(BlockId::Number(0));
        let to_block = filter.to_block.unwrap_or(BlockId::Tag(BlockTag::Latest));

        let events = self
            .sequencer
            .read()
            .await
            .events(
                from_block,
                to_block,
                filter.address.map(StarkFelt::from),
                filter.keys.map(|keys| {
                    keys.iter().map(|key| key.iter().map(|key| (*key).into()).collect()).collect()
                }),
                continuation_token,
                chunk_size,
            )
            .map_err(|e| match e {
                SequencerError::BlockNotFound(_) => StarknetApiError::BlockNotFound,
                _ => StarknetApiError::InternalServerError,
            })?;

        Ok(EventsPage {
            events: events
                .iter()
                .map(|e| EmittedEvent {
                    block_number: e.block_number.0,
                    block_hash: (e.block_hash.0).into(),
                    transaction_hash: (e.transaction_hash.0).into(),
                    from_address: (*e.inner.from_address.0.key()).into(),
                    keys: e.inner.content.keys.iter().map(|key| (key.0).into()).collect(),
                    data: e.inner.content.data.0.iter().map(|fe| (*fe).into()).collect(),
                })
                .collect(),
            continuation_token: None,
        })
    }

    async fn pending_transactions(&self) -> Result<Vec<Transaction>, Error> {
        Err(Error::from(StarknetApiError::InternalServerError))
    }

    async fn call(
        &self,
        request: FunctionCall,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, Error> {
        let call = ExternalFunctionCall {
            contract_address: ContractAddress(patricia_key!(request.contract_address)),
            calldata: Calldata(Arc::new(
                request.calldata.into_iter().map(StarkFelt::from).collect(),
            )),
            entry_point_selector: EntryPointSelector(StarkFelt::from(request.entry_point_selector)),
        };

        let res = self
            .sequencer
            .read()
            .await
            .call(block_id, call)
            .map_err(|_| Error::from(StarknetApiError::ContractError))?;

        let mut values = vec![];

        for f in res.into_iter() {
            values.push(f.into());
        }

        Ok(values)
    }

    async fn storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        block_id: BlockId,
    ) -> Result<FieldElement, Error> {
        let value = self
            .sequencer
            .write()
            .await
            .storage_at(
                ContractAddress(patricia_key!(contract_address)),
                StorageKey(patricia_key!(key)),
                block_id,
            )
            .map_err(|e| match e {
                SequencerError::StateNotFound(_) => Error::from(StarknetApiError::BlockNotFound),
                SequencerError::State(_) => Error::from(StarknetApiError::ContractNotFound),
                _ => Error::from(StarknetApiError::InternalServerError),
            })?;

        Ok(value.into())
    }

    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTransaction,
    ) -> Result<DeployAccountTransactionResult, Error> {
        let BroadcastedDeployAccountTransaction {
            max_fee,
            signature,
            nonce,
            contract_address_salt,
            constructor_calldata,
            class_hash,
        } = deploy_account_transaction;

        let (transaction_hash, contract_address) = self
            .sequencer
            .write()
            .await
            .deploy_account(
                ClassHash(StarkFelt::from(class_hash)),
                ContractAddressSalt(StarkFelt::from(contract_address_salt)),
                Calldata(Arc::new(constructor_calldata.into_iter().map(StarkFelt::from).collect())),
                TransactionSignature(signature.into_iter().map(StarkFelt::from).collect()),
            )
            .map_err(|e| Error::Call(CallError::Failed(anyhow::anyhow!(e.to_string()))))?;

        Ok(DeployAccountTransactionResult {
            transaction_hash: FieldElement::from(transaction_hash.0),
            contract_address: FieldElement::from(*contract_address.0.key()),
        })
    }

    async fn estimate_fee(
        &self,
        request: Vec<BroadcastedTransaction>,
        block_id: BlockId,
    ) -> Result<Vec<FeeEstimate>, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.read().await.chain_id().as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let mut res = Vec::new();

        for r in request {
            let transaction = match r {
                BroadcastedTransaction::Declare(BroadcastedDeclareTransaction::V2(tx)) => {
                    let raw_class_str = serde_json::to_string(&tx.contract_class)?;
                    let class_hash = serde_json::from_str::<FlattenedSierraClass>(&raw_class_str)
                        .map_err(|_| Error::from(StarknetApiError::InvalidContractClass))?
                        .class_hash();
                    let contract_class =
                        blockifier_contract_class_from_flattened_sierra_class(&raw_class_str)
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
                        transaction_hash: TransactionHash(StarkFelt::from(transaction_hash)),
                        class_hash: ClassHash(StarkFelt::from(class_hash)),
                        sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                        nonce: Nonce(StarkFelt::from(tx.nonce)),
                        max_fee: Fee(starkfelt_to_u128(StarkFelt::from(tx.max_fee))
                            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                        signature: TransactionSignature(
                            tx.signature.into_iter().map(StarkFelt::from).collect(),
                        ),
                        compiled_class_hash: CompiledClassHash(StarkFelt::from(
                            tx.compiled_class_hash,
                        )),
                    };

                    AccountTransaction::Declare(DeclareTransaction {
                        tx: starknet_api::transaction::DeclareTransaction::V2(transaction),
                        contract_class: blockifier::execution::contract_class::ContractClass::V1(
                            contract_class,
                        ),
                    })
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

                _ => return Err(Error::from(StarknetApiError::UnsupportedTransactionVersion)),
            };

            let fee_estimate =
                self.sequencer.read().await.estimate_fee(transaction, block_id).map_err(
                    |e| match e {
                        SequencerError::StateNotFound(_) => {
                            Error::from(StarknetApiError::BlockNotFound)
                        }
                        SequencerError::TransactionExecution(_) => {
                            Error::from(StarknetApiError::ContractError)
                        }
                        _ => Error::from(StarknetApiError::InternalServerError),
                    },
                )?;

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
        transaction: BroadcastedDeclareTransaction,
    ) -> Result<DeclareTransactionResult, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.read().await.chain_id().as_hex())
            .map_err(|_| Error::from(StarknetApiError::InternalServerError))?;

        let (transaction_hash, class_hash, transaction) = match transaction {
            BroadcastedDeclareTransaction::V1(_) => {
                return Err(Error::from(StarknetApiError::UnsupportedTransactionVersion));
            }
            BroadcastedDeclareTransaction::V2(tx) => {
                let raw_class_str = serde_json::to_string(&tx.contract_class)
                    .map_err(|_| Error::from(StarknetApiError::InvalidContractClass))?;
                let class_hash = serde_json::from_str::<FlattenedSierraClass>(&raw_class_str)
                    .map_err(|_| Error::from(StarknetApiError::InvalidContractClass))?
                    .class_hash();
                let contract_class =
                    blockifier_contract_class_from_flattened_sierra_class(&raw_class_str)
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
                    transaction_hash: TransactionHash(StarkFelt::from(transaction_hash)),
                    class_hash: ClassHash(StarkFelt::from(class_hash)),
                    sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                    nonce: Nonce(StarkFelt::from(tx.nonce)),
                    max_fee: Fee(starkfelt_to_u128(StarkFelt::from(tx.max_fee))
                        .map_err(|_| Error::from(StarknetApiError::InternalServerError))?),
                    signature: TransactionSignature(
                        tx.signature.into_iter().map(StarkFelt::from).collect(),
                    ),
                    compiled_class_hash: CompiledClassHash(StarkFelt::from(tx.compiled_class_hash)),
                };

                (
                    transaction_hash,
                    class_hash,
                    AccountTransaction::Declare(DeclareTransaction {
                        tx: starknet_api::transaction::DeclareTransaction::V2(transaction),
                        contract_class: blockifier::execution::contract_class::ContractClass::V1(
                            contract_class,
                        ),
                    }),
                )
            }
        };

        self.sequencer.write().await.add_account_transaction(transaction);

        Ok(DeclareTransactionResult { transaction_hash, class_hash })
    }

    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTransaction,
    ) -> Result<InvokeTransactionResult, Error> {
        match invoke_transaction {
            BroadcastedInvokeTransaction::V1(transaction) => {
                let chain_id =
                    FieldElement::from_hex_be(&self.sequencer.read().await.chain_id().as_hex())
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

                self.sequencer.write().await.add_account_transaction(AccountTransaction::Invoke(
                    InvokeTransaction::V1(transaction),
                ));

                Ok(InvokeTransactionResult { transaction_hash })
            }

            _ => Err(Error::from(StarknetApiError::UnsupportedTransactionVersion)),
        }
    }
}
