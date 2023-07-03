use std::str::FromStr;
use std::sync::Arc;

use blockifier::state::errors::StateError;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::transactions::DeclareTransaction;
use jsonrpsee::core::{async_trait, Error};
use jsonrpsee::types::error::CallError;
use katana_core::constants::SEQUENCER_ADDRESS;
use katana_core::sequencer::Sequencer;
use katana_core::sequencer_error::SequencerError;
use katana_core::starknet::contract::StarknetContract;
use katana_core::starknet::transaction::ExternalFunctionCall;
use katana_core::util::starkfelt_to_u128;
use starknet::core::types::{
    BlockHashAndNumber, BlockId, BlockStatus, BlockTag, BlockWithTxHashes, BlockWithTxs,
    BroadcastedDeclareTransaction, BroadcastedDeployAccountTransaction,
    BroadcastedInvokeTransaction, BroadcastedTransaction, ContractClass, DeclareTransactionReceipt,
    DeclareTransactionResult, DeployAccountTransactionReceipt, DeployAccountTransactionResult,
    DeployTransactionReceipt, EmittedEvent, Event, EventFilterWithPage, EventsPage, FeeEstimate,
    FieldElement, FunctionCall, InvokeTransactionReceipt, InvokeTransactionResult,
    MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs, MaybePendingTransactionReceipt,
    MsgToL1, PendingBlockWithTxHashes, PendingBlockWithTxs, PendingDeclareTransactionReceipt,
    PendingDeployAccountTransactionReceipt, PendingInvokeTransactionReceipt,
    PendingTransactionReceipt, StateUpdate, Transaction, TransactionReceipt, TransactionStatus,
};
use starknet_api::core::{
    ClassHash, CompiledClassHash, ContractAddress, EntryPointSelector, Nonce, PatriciaKey,
};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeclareTransactionV0V1, DeclareTransactionV2, Fee,
    InvokeTransaction, InvokeTransactionV1, Transaction as InnerTransaction, TransactionHash,
    TransactionOutput, TransactionSignature,
};
use utils::transaction::{
    compute_declare_v1_transaction_hash, compute_declare_v2_transaction_hash,
    compute_invoke_v1_transaction_hash, convert_inner_to_rpc_tx,
};

use crate::api::starknet::{Felt, StarknetApiError, StarknetApiServer};
use crate::utils;
use crate::utils::contract::{
    legacy_inner_to_rpc_class, legacy_rpc_to_inner_class, rpc_to_inner_class,
};

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
        Ok(self.sequencer.block_number().await.0)
    }

    async fn transaction_by_hash(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<Transaction, Error> {
        let tx = self
            .sequencer
            .transaction(&TransactionHash(StarkFelt::from(transaction_hash)))
            .await
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;

        convert_inner_to_rpc_tx(tx).map_err(|_| Error::from(StarknetApiError::InternalServerError))
    }

    async fn block_transaction_count(&self, block_id: BlockId) -> Result<u64, Error> {
        let block = self
            .sequencer
            .block(block_id)
            .await
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
        let class_hash = self.class_hash_at(block_id, contract_address).await?;
        self.class(block_id, class_hash.0).await
    }

    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error> {
        let (hash, number) = self
            .sequencer
            .block_hash_and_number()
            .await
            .ok_or(Error::from(StarknetApiError::NoBlocks))?;

        Ok(BlockHashAndNumber { block_number: number.0, block_hash: hash.0.into() })
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
            .block(block_id)
            .await
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
            .block(block_id)
            .await
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
            .state_update(block_id)
            .await
            .map_err(|_| Error::from(StarknetApiError::BlockNotFound))
    }

    async fn transaction_receipt(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<MaybePendingTransactionReceipt, Error> {
        let hash = TransactionHash(StarkFelt::from(transaction_hash));

        let tx = self
            .sequencer
            .transaction(&hash)
            .await
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;

        let receipt = self
            .sequencer
            .transaction_receipt(&hash)
            .await
            .ok_or(Error::from(StarknetApiError::TxnHashNotFound))?;

        let status = self
            .sequencer
            .transaction_status(&hash)
            .await
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

                TransactionOutput::Deploy(output) => MaybePendingTransactionReceipt::Receipt(
                    TransactionReceipt::Deploy(DeployTransactionReceipt {
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
                            InnerTransaction::Deploy(tx) => (*tx.contract_address.0.key()).into(),
                            _ => {
                                return Err(Error::from(StarknetApiError::InternalServerError));
                            }
                        },
                    }),
                ),

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
        let keys = {
            if let Some(keys) = keys {
                if keys.len() == 1 && keys.is_empty() {
                    None
                } else {
                    Some(
                        keys.iter()
                            .map(|key| key.iter().map(|key| (*key).into()).collect())
                            .collect(),
                    )
                }
            } else {
                None
            }
        };

        let events = self
            .sequencer
            .events(
                from_block,
                to_block,
                filter.event_filter.address.map(StarkFelt::from),
                keys,
                filter.result_page_request.continuation_token,
                filter.result_page_request.chunk_size,
            )
            .await
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
        let block = self.sequencer.block(BlockId::Tag(BlockTag::Pending)).await;

        match block {
            Some(block) => {
                let txs: anyhow::Result<_> =
                    block.transactions().iter().try_fold(Vec::new(), |mut data, tx| {
                        data.push(convert_inner_to_rpc_tx(tx.clone())?);
                        Ok(data)
                    });

                match txs {
                    Ok(txs) => Ok(txs),
                    Err(_) => Err(Error::from(StarknetApiError::InternalServerError)),
                }
            }
            None => Ok(vec![]),
        }
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
        let BroadcastedDeployAccountTransaction {
            signature,
            contract_address_salt,
            constructor_calldata,
            class_hash,
            ..
        } = deploy_account_transaction;

        let (transaction_hash, contract_address) = self
            .sequencer
            .deploy_account(
                ClassHash(StarkFelt::from(class_hash)),
                ContractAddressSalt(StarkFelt::from(contract_address_salt)),
                Calldata(Arc::new(constructor_calldata.into_iter().map(StarkFelt::from).collect())),
                TransactionSignature(signature.into_iter().map(StarkFelt::from).collect()),
            )
            .await
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
