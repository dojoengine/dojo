use blockifier::transaction::{
    account_transaction::AccountTransaction, transactions::DeclareTransaction,
};
use config::RpcConfig;
use jsonrpsee::{
    core::{async_trait, Error},
    server::{ServerBuilder, ServerHandle},
    types::error::CallError,
};
use katana_core::{
    constants::SEQUENCER_ADDRESS,
    sequencer::Sequencer,
    starknet::transaction::ExternalFunctionCall,
    util::{
        blockifier_contract_class_from_flattened_sierra_class, field_element_to_starkfelt,
        starkfelt_to_u128,
    },
};
use starknet::providers::jsonrpc::models::{
    BlockHashAndNumber, BlockId, BlockStatus, BlockWithTxHashes, BlockWithTxs,
    BroadcastedDeclareTransaction, BroadcastedDeployAccountTransaction,
    BroadcastedInvokeTransaction, BroadcastedTransaction, ContractClass, DeclareTransactionResult,
    DeployAccountTransactionResult, EmittedEvent, EventFilter, EventsPage, FeeEstimate,
    FunctionCall, InvokeTransactionResult, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    MaybePendingTransactionReceipt, PendingBlockWithTxs, StateUpdate, Transaction,
};
use starknet::{core::types::contract::FlattenedSierraClass, providers::jsonrpc::models::BlockTag};
use starknet::{core::types::FieldElement, providers::jsonrpc::models::PendingBlockWithTxHashes};
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, PatriciaKey},
    hash::StarkFelt,
    transaction::{
        Calldata, ContractAddressSalt, DeclareTransactionV2, Fee, InvokeTransaction,
        TransactionVersion,
    },
};
use starknet_api::{
    core::{EntryPointSelector, Nonce},
    patricia_key,
    transaction::TransactionHash,
};
use starknet_api::{hash::StarkHash, transaction::TransactionSignature};
use starknet_api::{state::StorageKey, transaction::InvokeTransactionV1};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use utils::transaction::{
    compute_declare_v2_transaction_hash, compute_invoke_v1_transaction_hash,
    convert_inner_to_rpc_tx, stark_felt_to_field_element,
};

pub mod api;
pub mod config;
pub mod utils;

use api::{KatanaApiError, KatanaApiServer, KatanaRpcLogger};

pub struct KatanaRpc<S> {
    pub config: RpcConfig,
    pub sequencer: Arc<RwLock<S>>,
}

impl<S> KatanaRpc<S>
where
    S: Sequencer + Send + Sync + 'static,
{
    pub fn new(sequencer: Arc<RwLock<S>>, config: RpcConfig) -> Self {
        Self { config, sequencer }
    }

    pub async fn run(self) -> Result<(SocketAddr, ServerHandle), Error> {
        let server = ServerBuilder::new()
            .set_logger(KatanaRpcLogger)
            .build(format!("127.0.0.1:{}", self.config.port))
            .await
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

        let addr = server.local_addr()?;
        let handle = server.start(self.into_rpc())?;

        Ok((addr, handle))
    }
}

#[allow(unused)]
#[async_trait]
impl<S: Sequencer + Send + Sync + 'static> KatanaApiServer for KatanaRpc<S> {
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
            .nonce_at(
                block_id,
                ContractAddress(patricia_key!(field_element_to_starkfelt(&contract_address))),
            )
            .map_err(|_| Error::from(KatanaApiError::ContractError))?;

        stark_felt_to_field_element(nonce.0)
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))
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
            .transaction(&TransactionHash(field_element_to_starkfelt(
                &transaction_hash,
            )))
            .ok_or(Error::from(KatanaApiError::TxnHashNotFound))?;

        convert_inner_to_rpc_tx(tx).map_err(|_| Error::from(KatanaApiError::InternalServerError))
    }

    async fn block_transaction_count(&self, block_id: BlockId) -> Result<u64, Error> {
        let block = self
            .sequencer
            .read()
            .await
            .block(block_id.clone())
            .ok_or(Error::from(KatanaApiError::BlockNotFound))?;

        block
            .transactions()
            .len()
            .try_into()
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))
    }

    async fn class_at(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<ContractClass, Error> {
        unimplemented!("KatanaRpc::class_at")
    }

    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error> {
        unimplemented!("KatanaRpc::block_hash_and_number")
    }

    async fn block_with_tx_hashes(
        &self,
        block_id: BlockId,
    ) -> Result<MaybePendingBlockWithTxHashes, Error> {
        let block = self
            .sequencer
            .read()
            .await
            .block(block_id.clone())
            .ok_or(Error::from(KatanaApiError::BlockNotFound))?;

        let sequencer_address = FieldElement::from_hex_be(SEQUENCER_ADDRESS).unwrap();
        let transactions = block
            .transactions()
            .iter()
            .map(|tx| stark_felt_to_field_element(tx.transaction_hash().0).unwrap())
            .collect::<Vec<_>>();

        let timestamp = block.header().timestamp.0;
        let parent_hash = stark_felt_to_field_element(block.header().parent_hash.0).unwrap();

        if BlockId::Tag(BlockTag::Pending) == block_id {
            return Ok(MaybePendingBlockWithTxHashes::PendingBlock(
                PendingBlockWithTxHashes {
                    transactions,
                    sequencer_address,
                    timestamp,
                    parent_hash,
                },
            ));
        }

        Ok(MaybePendingBlockWithTxHashes::Block(BlockWithTxHashes {
            new_root: stark_felt_to_field_element(block.header().state_root.0).unwrap(),
            block_hash: stark_felt_to_field_element(block.header().block_hash.0).unwrap(),
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
            .block(block_id.clone())
            .ok_or(Error::from(KatanaApiError::BlockNotFound))?;

        let transaction = block
            .transactions()
            .get(index)
            .ok_or(Error::from(KatanaApiError::InvalidTxnIndex))?;

        convert_inner_to_rpc_tx(transaction.clone())
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))
    }

    async fn block_with_txs(&self, block_id: BlockId) -> Result<MaybePendingBlockWithTxs, Error> {
        let block = self
            .sequencer
            .read()
            .await
            .block(block_id.clone())
            .ok_or(Error::from(KatanaApiError::BlockNotFound))?;

        let sequencer_address = FieldElement::from_hex_be(SEQUENCER_ADDRESS).unwrap();
        let transactions = block
            .transactions()
            .iter()
            .map(|tx| convert_inner_to_rpc_tx(tx.clone()).unwrap())
            .collect::<Vec<_>>();
        let timestamp = block.header().timestamp.0;
        let parent_hash = stark_felt_to_field_element(block.header().parent_hash.0).unwrap();

        if BlockId::Tag(BlockTag::Pending) == block_id {
            return Ok(MaybePendingBlockWithTxs::PendingBlock(
                PendingBlockWithTxs {
                    transactions,
                    sequencer_address,
                    timestamp,
                    parent_hash,
                },
            ));
        }

        Ok(MaybePendingBlockWithTxs::Block(BlockWithTxs {
            new_root: stark_felt_to_field_element(block.header().state_root.0).unwrap(),
            block_hash: stark_felt_to_field_element(block.block_hash().0).unwrap(),
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
            .map_err(|_| Error::from(KatanaApiError::BlockNotFound))
    }

    async fn transaction_receipt(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<MaybePendingTransactionReceipt, Error> {
        unimplemented!("KatanaRpc::transaction_receipt")
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
            .class_hash_at(
                block_id,
                ContractAddress(patricia_key!(field_element_to_starkfelt(&contract_address))),
            )
            .map_err(|_| Error::from(KatanaApiError::ContractError))?;

        stark_felt_to_field_element(class_hash.0)
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))
    }

    async fn class(
        &self,
        block_id: BlockId,
        class_hash: FieldElement,
    ) -> Result<ContractClass, Error> {
        unimplemented!("KatanaRpc::class")
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
                filter.address.map(|fe| field_element_to_starkfelt(&fe)),
                filter
                    .keys
                    .map(|keys| keys.iter().map(field_element_to_starkfelt).collect()),
                continuation_token,
                chunk_size,
            )
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

        Ok(EventsPage {
            events: events
                .iter()
                .map(|e| EmittedEvent {
                    block_number: e.block_number.0,
                    block_hash: stark_felt_to_field_element(e.block_hash.0).unwrap(),
                    transaction_hash: stark_felt_to_field_element(e.transaction_hash.0).unwrap(),
                    from_address: stark_felt_to_field_element(*e.inner.from_address.0.key())
                        .unwrap(),
                    keys: e
                        .inner
                        .content
                        .keys
                        .iter()
                        .map(|key| stark_felt_to_field_element(key.0).unwrap())
                        .collect(),
                    data: e
                        .inner
                        .content
                        .data
                        .0
                        .iter()
                        .map(|fe| stark_felt_to_field_element(*fe).unwrap())
                        .collect(),
                })
                .collect(),
            continuation_token: None,
        })
    }

    async fn pending_transactions(&self) -> Result<Vec<Transaction>, Error> {
        unimplemented!("KatanaRpc::pending_transactions")
    }

    async fn estimate_fee(
        &self,
        request: BroadcastedTransaction,
        block_id: BlockId,
    ) -> Result<FeeEstimate, Error> {
        unimplemented!("KatanaRpc::estimate_fee")
    }

    async fn call(
        &self,
        request: FunctionCall,
        block_id: BlockId,
    ) -> Result<Vec<FieldElement>, Error> {
        let call = ExternalFunctionCall {
            contract_address: ContractAddress(patricia_key!(field_element_to_starkfelt(
                &request.contract_address
            ))),
            calldata: Calldata(Arc::new(
                request
                    .calldata
                    .iter()
                    .map(field_element_to_starkfelt)
                    .collect(),
            )),
            entry_point_selector: EntryPointSelector(field_element_to_starkfelt(
                &request.entry_point_selector,
            )),
        };

        let res = self
            .sequencer
            .read()
            .await
            .call(block_id, call)
            .map_err(|_| Error::from(KatanaApiError::ContractError))?;

        let mut values = vec![];

        for f in res.into_iter() {
            values.push(
                stark_felt_to_field_element(f)
                    .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
            );
        }

        Ok(values)
    }

    async fn storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        _block_id: BlockId,
    ) -> Result<FieldElement, Error> {
        let value = self
            .sequencer
            .write()
            .await
            .storage_at(
                ContractAddress(patricia_key!(field_element_to_starkfelt(&contract_address))),
                StorageKey(patricia_key!(field_element_to_starkfelt(&key))),
            )
            .map_err(|_| Error::from(KatanaApiError::ContractError))?;

        stark_felt_to_field_element(value)
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))
    }

    async fn add_deploy_account_transaction(
        &self,
        deploy_account_transaction: BroadcastedDeployAccountTransaction,
    ) -> Result<DeployAccountTransactionResult, Error> {
        let BroadcastedDeployAccountTransaction {
            max_fee,
            version,
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
                ClassHash(field_element_to_starkfelt(&class_hash)),
                TransactionVersion(StarkFelt::from(version)),
                ContractAddressSalt(field_element_to_starkfelt(&contract_address_salt)),
                Calldata(Arc::new(
                    constructor_calldata
                        .iter()
                        .map(field_element_to_starkfelt)
                        .collect(),
                )),
                TransactionSignature(signature.iter().map(field_element_to_starkfelt).collect()),
            )
            .map_err(|e| Error::Call(CallError::Failed(anyhow::anyhow!(e.to_string()))))?;

        Ok(DeployAccountTransactionResult {
            transaction_hash: FieldElement::from_byte_slice_be(transaction_hash.0.bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
            contract_address: FieldElement::from_byte_slice_be(contract_address.0.key().bytes())
                .map_err(|_| Error::from(KatanaApiError::InternalServerError))?,
        })
    }

    async fn add_declare_transaction(
        &self,
        transaction: BroadcastedDeclareTransaction,
    ) -> Result<DeclareTransactionResult, Error> {
        let chain_id = FieldElement::from_hex_be(&self.sequencer.read().await.chain_id().as_hex())
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

        let (transaction_hash, class_hash, transaction) = match transaction {
            BroadcastedDeclareTransaction::V1(_) => {
                unimplemented!("KatanaRpc::add_declare_transaction v1")
            }
            BroadcastedDeclareTransaction::V2(tx) => {
                let raw_class_str = serde_json::to_string(&tx.contract_class)?;
                let class_hash = serde_json::from_str::<FlattenedSierraClass>(&raw_class_str)
                    .map_err(|_| Error::from(KatanaApiError::InvalidContractClass))?
                    .class_hash();
                let contract_class =
                    blockifier_contract_class_from_flattened_sierra_class(&raw_class_str)
                        .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

                let transaction_hash = compute_declare_v2_transaction_hash(
                    tx.sender_address,
                    class_hash,
                    tx.max_fee,
                    chain_id,
                    tx.nonce,
                    tx.compiled_class_hash,
                );

                let transaction = DeclareTransactionV2 {
                    transaction_hash: TransactionHash(field_element_to_starkfelt(
                        &transaction_hash,
                    )),
                    class_hash: ClassHash(field_element_to_starkfelt(&class_hash)),
                    sender_address: ContractAddress(patricia_key!(field_element_to_starkfelt(
                        &tx.sender_address
                    ))),
                    nonce: Nonce(field_element_to_starkfelt(&tx.nonce)),
                    max_fee: Fee(starkfelt_to_u128(field_element_to_starkfelt(&tx.max_fee))
                        .map_err(|_| Error::from(KatanaApiError::InternalServerError))?),
                    signature: TransactionSignature(
                        tx.signature
                            .iter()
                            .map(field_element_to_starkfelt)
                            .collect(),
                    ),
                    compiled_class_hash: CompiledClassHash(field_element_to_starkfelt(
                        &tx.compiled_class_hash,
                    )),
                };

                (
                    transaction_hash,
                    class_hash,
                    AccountTransaction::Declare(DeclareTransaction {
                        tx: starknet_api::transaction::DeclareTransaction::V2(transaction),
                        contract_class,
                    }),
                )
            }
        };

        self.sequencer
            .write()
            .await
            .add_account_transaction(transaction);

        Ok(DeclareTransactionResult {
            transaction_hash,
            class_hash,
        })
    }

    async fn add_invoke_transaction(
        &self,
        invoke_transaction: BroadcastedInvokeTransaction,
    ) -> Result<InvokeTransactionResult, Error> {
        match invoke_transaction {
            BroadcastedInvokeTransaction::V1(transaction) => {
                let chain_id =
                    FieldElement::from_hex_be(&self.sequencer.read().await.chain_id().as_hex())
                        .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

                let transaction_hash = compute_invoke_v1_transaction_hash(
                    transaction.sender_address,
                    &transaction.calldata,
                    transaction.max_fee,
                    chain_id,
                    transaction.nonce,
                );

                let transaction = InvokeTransactionV1 {
                    transaction_hash: TransactionHash(field_element_to_starkfelt(
                        &transaction_hash,
                    )),
                    sender_address: ContractAddress(patricia_key!(field_element_to_starkfelt(
                        &transaction.sender_address
                    ))),
                    nonce: Nonce(field_element_to_starkfelt(&transaction.nonce)),
                    calldata: Calldata(Arc::new(
                        transaction
                            .calldata
                            .iter()
                            .map(field_element_to_starkfelt)
                            .collect(),
                    )),
                    max_fee: Fee(starkfelt_to_u128(field_element_to_starkfelt(
                        &transaction.max_fee,
                    ))
                    .map_err(|_| Error::from(KatanaApiError::InternalServerError))?),
                    signature: TransactionSignature(
                        transaction
                            .signature
                            .iter()
                            .map(field_element_to_starkfelt)
                            .collect(),
                    ),
                };

                self.sequencer
                    .write()
                    .await
                    .add_account_transaction(AccountTransaction::Invoke(InvokeTransaction::V1(
                        transaction,
                    )));

                Ok(InvokeTransactionResult { transaction_hash })
            }

            _ => Err(Error::from(KatanaApiError::InternalServerError)),
        }
    }
}
