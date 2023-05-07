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
    sequencer::Sequencer,
    starknet::transaction::ExternalFunctionCall,
    util::{field_element_to_starkfelt, starkfelt_to_u128},
};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::models::{
    BlockHashAndNumber, BlockId, BroadcastedDeclareTransaction,
    BroadcastedDeployAccountTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    ContractClass, DeclareTransactionResult, DeployAccountTransactionResult, EventFilter,
    EventsPage, FeeEstimate, FunctionCall, InvokeTransactionResult, MaybePendingBlockWithTxHashes,
    MaybePendingBlockWithTxs, MaybePendingTransactionReceipt, StateUpdate, Transaction,
};
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, PatriciaKey},
    hash::StarkFelt,
    transaction::{
        Calldata, ContractAddressSalt, DeclareTransactionV0V1, DeclareTransactionV2, Fee,
        InvokeTransaction, TransactionVersion,
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
use util::{
    compute_invoke_v1_transaction_hash, convert_inner_to_rpc_tx, get_blockifier_contract_class,
    get_legacy_contract_class_hash, get_sierra_class_hash, stark_felt_to_field_element,
};

pub mod api;
pub mod config;
pub mod util;

use api::{KatanaApiError, KatanaApiServer, KatanaRpcLogger};

use crate::util::{compute_declare_v1_transaction_hash, compute_declare_v2_transaction_hash};

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

    async fn get_nonce(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<FieldElement, Error> {
        let nonce = self
            .sequencer
            .write()
            .await
            .get_nonce_at(
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

    async fn get_transaction_by_hash(
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

    async fn get_block_transaction_count(&self, block_id: BlockId) -> Result<u64, Error> {
        unimplemented!("KatanaRpc::get_block_transaction_count")
    }

    async fn get_class_at(
        &self,
        block_id: BlockId,
        contract_address: FieldElement,
    ) -> Result<ContractClass, Error> {
        unimplemented!("KatanaRpc::get_class_at")
    }

    async fn block_hash_and_number(&self) -> Result<BlockHashAndNumber, Error> {
        unimplemented!("KatanaRpc::block_hash_and_number")
    }

    async fn get_block_with_tx_hashes(
        &self,
        block_id: BlockId,
    ) -> Result<MaybePendingBlockWithTxHashes, Error> {
        unimplemented!("KatanaRpc::get_block_with_tx_hashes")
    }

    async fn get_transaction_by_block_id_and_index(
        &self,
        block_id: BlockId,
        index: usize,
    ) -> Result<Transaction, Error> {
        unimplemented!("KatanaRpc::get_transaction_by_block_id_and_index")
    }

    async fn get_block_with_txs(
        &self,
        block_id: BlockId,
    ) -> Result<MaybePendingBlockWithTxs, Error> {
        unimplemented!("KatanaRpc::get_block_with_txs")
    }

    async fn get_state_update(&self, block_id: BlockId) -> Result<StateUpdate, Error> {
        unimplemented!("KatanaRpc::get_state_update")
    }

    async fn get_transaction_receipt(
        &self,
        transaction_hash: FieldElement,
    ) -> Result<MaybePendingTransactionReceipt, Error> {
        unimplemented!("KatanaRpc::get_transaction_receipt")
    }

    async fn get_class_hash_at(
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

    async fn get_class(
        &self,
        block_id: BlockId,
        class_hash: FieldElement,
    ) -> Result<ContractClass, Error> {
        unimplemented!("KatanaRpc::get_class")
    }

    async fn get_events(
        &self,
        filter: EventFilter,
        continuation_token: Option<String>,
        chunk_size: u64,
    ) -> Result<EventsPage, Error> {
        unimplemented!("KatanaRpc::get_events")
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

    async fn get_storage_at(
        &self,
        contract_address: FieldElement,
        key: FieldElement,
        _block_id: BlockId,
    ) -> Result<FieldElement, Error> {
        let value = self
            .sequencer
            .write()
            .await
            .get_storage_at(
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
            BroadcastedDeclareTransaction::V1(tx) => {
                let raw_class_str = serde_json::to_string(&tx.contract_class)?;
                let class_hash = get_legacy_contract_class_hash(&raw_class_str)
                    .map_err(|_| Error::from(KatanaApiError::InvalidContractClass))?;

                let transaction_hash = compute_declare_v1_transaction_hash(
                    tx.sender_address,
                    class_hash,
                    tx.max_fee,
                    chain_id,
                    tx.nonce,
                );

                let transaction = DeclareTransactionV0V1 {
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
                };

                (
                    transaction_hash,
                    class_hash,
                    AccountTransaction::Declare(DeclareTransaction {
                        tx: starknet_api::transaction::DeclareTransaction::V1(transaction),
                        contract_class: Default::default(),
                    }),
                )
            }
            BroadcastedDeclareTransaction::V2(tx) => {
                let raw_class_str = serde_json::to_string(&tx.contract_class)?;
                let class_hash = get_sierra_class_hash(&raw_class_str)
                    .map_err(|_| Error::from(KatanaApiError::InvalidContractClass))?;
                let contract_class = get_blockifier_contract_class(&raw_class_str)
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
