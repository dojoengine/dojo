use async_trait::async_trait;
use beerus_core::lightclient::beerus::BeerusLightClient;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::system::IntoPipeSystem;
use bevy_tokio_tasks::TaskContext;
use dojo_macros::LightClientSystem;
use ethabi::Uint as U256;
use eyre::Result;
use starknet::core::types::{
    BlockId, BroadcastedDeclareTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    EventFilter, FieldElement, FunctionCall,
};

use crate::light_client::{handle_request_error, BlockNumber, RequestSender};

pub struct StarknetClientPlugin;

impl Plugin for StarknetClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<StarknetGetBlockWithTxHashes>()
            .add_event::<StarknetGetBlockWithTxs>()
            .add_event::<StarknetGetStateUpdate>()
            .add_event::<StarknetGetStorageAt>()
            .add_event::<StarknetGetTransactionByHash>()
            .add_event::<StarknetGetTransactionByBlockIdAndIndex>()
            .add_event::<StarknetGetTransactionReceipt>()
            .add_event::<StarknetGetClass>()
            .add_event::<StarknetGetClassHashAt>()
            .add_event::<StarknetGetClassAt>()
            .add_event::<StarknetGetBlockTransactionCount>()
            .add_event::<StarknetCall>()
            .add_event::<StarknetEstimateFee>()
            .add_event::<StarknetBlockNumber>()
            .add_event::<StarknetBlockHashAndNumber>()
            .add_event::<StarknetChainId>()
            .add_event::<StarknetPendingTransactions>()
            .add_event::<StarknetSyncing>()
            .add_event::<StarknetGetEvents>()
            .add_event::<StarknetGetNonce>()
            .add_event::<StarknetL1ToL2Messages>()
            .add_event::<StarknetL1ToL2MessageNonce>()
            .add_event::<StarknetL1ToL2MessageCancellations>()
            .add_event::<StarknetL2ToL1Messages>()
            .add_event::<StarknetAddDeclareTransaction>()
            .add_event::<StarknetAddDeployAccountTransaction>()
            .add_event::<StarknetGetContractStorageProof>()
            .add_event::<StarknetAddInvokeTransaction>()
            // add_systems supports up to 15 systems each
            .add_systems((
                get_block_with_tx_hashes.pipe(handle_request_error),
                get_block_with_txs.pipe(handle_request_error),
                get_state_update.pipe(handle_request_error),
                get_storage_at.pipe(handle_request_error),
                get_transaction_by_hash.pipe(handle_request_error),
                get_transaction_by_block_id_and_index.pipe(handle_request_error),
                get_transaction_receipt.pipe(handle_request_error),
                get_class.pipe(handle_request_error),
                get_class_hash_at.pipe(handle_request_error),
                get_class_at.pipe(handle_request_error),
                get_block_transaction_count.pipe(handle_request_error),
                call.pipe(handle_request_error),
                estimate_fee.pipe(handle_request_error),
                block_number.pipe(handle_request_error),
                block_hash_and_number.pipe(handle_request_error),
            ))
            .add_systems((
                chain_id.pipe(handle_request_error),
                pending_transactions.pipe(handle_request_error),
                syncing.pipe(handle_request_error),
                get_events.pipe(handle_request_error),
                get_nonce.pipe(handle_request_error),
            ))
            .add_systems((
                l1_to_l2_messages.pipe(handle_request_error),
                l1_to_l2_message_nonce.pipe(handle_request_error),
                l1_to_l2_message_cancellations.pipe(handle_request_error),
                l2_to_l1_messages.pipe(handle_request_error),
                add_declare_transaction.pipe(handle_request_error),
                add_deploy_account_transaction.pipe(handle_request_error),
                get_contract_storage_proof.pipe(handle_request_error),
                add_invoke_transaction.pipe(handle_request_error),
            ));
    }
}

////////////////////////////////////////////////////////////////////////
// Events
////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetBlockWithTxHashes {
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetBlockWithTxs {
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetStateUpdate {
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetStorageAt {
    pub address: FieldElement,
    pub key: FieldElement,
    pub block_number: u64,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetTransactionByHash {
    pub hash: FieldElement,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetTransactionByBlockIdAndIndex {
    pub block_id: BlockId,
    pub index: u64,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetTransactionReceipt {
    pub hash: FieldElement,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetClass {
    pub block_id: BlockId,
    pub class_hash: FieldElement,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetClassHashAt {
    pub block_id: BlockId,
    pub contract_address: FieldElement,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetClassAt {
    pub block_id: BlockId,
    pub contract_address: FieldElement,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetBlockTransactionCount {
    pub block_id: BlockId,
}

#[derive(Clone, Debug, LightClientSystem)]
pub struct StarknetCall {
    pub opts: FunctionCall,
    pub block_number: u64,
}

#[derive(Clone, Debug, LightClientSystem)]
pub struct StarknetEstimateFee {
    pub tx: BroadcastedTransaction,
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetBlockNumber;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetBlockHashAndNumber;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetChainId;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetPendingTransactions;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetSyncing;

#[derive(Clone, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetGetEvents {
    pub filter: EventFilter,
    pub continuation_token: Option<String>,
    pub chunk_size: u64,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetGetNonce {
    pub block_id: BlockId,
    pub address: FieldElement,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetL1ToL2Messages {
    pub msg_hash: U256,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetL1ToL2MessageNonce;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetL1ToL2MessageCancellations {
    pub msg_hash: U256,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct StarknetL2ToL1Messages {
    pub msg_hash: U256,
}

#[derive(Clone, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddDeclareTransaction {
    pub declare_transaction: BroadcastedDeclareTransaction,
}

#[derive(Clone, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddDeployAccountTransaction {
    pub contract_class: String,
    pub version: String,
    pub contract_address_salt: String,
    pub constructor_calldata: Vec<String>,
}

#[derive(Clone, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetGetContractStorageProof {
    pub block_id: BlockId,
    pub contract_address: String,
    pub keys: Vec<String>,
}

#[derive(Clone, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddInvokeTransaction {
    pub invoke_transaction: BroadcastedInvokeTransaction,
}

#[derive(Debug)]
pub enum StarknetRequest {
    GetBlockWithTxHashes(StarknetGetBlockWithTxHashes),
    GetBlockWithTxs(StarknetGetBlockWithTxs),
    GetStateUpdate(StarknetGetStateUpdate),
    GetStorageAt(StarknetGetStorageAt),
    GetTransactionByHash(StarknetGetTransactionByHash),
    GetTransactionByBlockIdAndIndex(StarknetGetTransactionByBlockIdAndIndex),
    GetTransactionReceipt(StarknetGetTransactionReceipt),
    GetClass(StarknetGetClass),
    GetClassHashAt(StarknetGetClassHashAt),
    GetClassAt(StarknetGetClassAt),
    GetBlockTransactionCount(StarknetGetBlockTransactionCount),
    Call(StarknetCall),
    EstimateFee(StarknetEstimateFee),
    BlockNumber,
    BlockHashAndNumber,
    ChainId,
    PendingTransactions,
    Syncing,
    GetEvents(StarknetGetEvents),
    GetNonce(StarknetGetNonce),
    L1ToL2Messages(StarknetL1ToL2Messages),
    L1ToL2MessageNonce,
    L1ToL2MessageCancellations(StarknetL1ToL2MessageCancellations),
    L2ToL1Messages(StarknetL2ToL1Messages),
    AddDeclareTransaction(StarknetAddDeclareTransaction),
    AddDeployAccountTransaction(StarknetAddDeployAccountTransaction),
    GetContractStorageProof(StarknetGetContractStorageProof),
    AddInvokeTransaction(StarknetAddInvokeTransaction),
}

#[async_trait]
impl RequestSender for StarknetRequest {
    async fn send(&self, client: &BeerusLightClient, ctx: TaskContext) -> Result<()> {
        use StarknetRequest::*;

        match self {
            GetBlockWithTxHashes(params) => {
                Self::get_block_with_tx_hashes(&client, ctx, params).await
            }
            GetBlockWithTxs(params) => {
                StarknetRequest::get_block_with_txs(&client, ctx, params).await
            }
            GetStateUpdate(params) => StarknetRequest::get_state_update(&client, ctx, params).await,
            GetStorageAt(params) => StarknetRequest::get_storage_at(&client, ctx, params).await,
            GetTransactionByHash(params) => {
                StarknetRequest::get_transaction_by_hash(&client, ctx, params).await
            }
            GetTransactionByBlockIdAndIndex(params) => {
                StarknetRequest::get_transaction_by_block_id_and_index(&client, ctx, params).await
            }
            GetTransactionReceipt(params) => {
                StarknetRequest::get_transaction_receipt(&client, ctx, params).await
            }
            GetClass(params) => StarknetRequest::get_class(&client, ctx, params).await,
            GetClassHashAt(params) => {
                StarknetRequest::get_class_hash_at(&client, ctx, params).await
            }
            GetClassAt(params) => StarknetRequest::get_class_at(&client, ctx, params).await,
            GetBlockTransactionCount(params) => {
                StarknetRequest::get_block_transaction_count(&client, ctx, params).await
            }
            Call(params) => StarknetRequest::get_call(&client, ctx, params).await,
            EstimateFee(params) => StarknetRequest::estimate_fee(&client, ctx, params).await,
            BlockNumber => StarknetRequest::block_number(&client, ctx).await,
            BlockHashAndNumber => StarknetRequest::block_hash_and_number(&client, ctx).await,
            ChainId => StarknetRequest::chain_id(&client, ctx).await,
            PendingTransactions => StarknetRequest::pending_transactions(&client, ctx).await,
            Syncing => StarknetRequest::syncing(&client, ctx).await,
            GetEvents(params) => StarknetRequest::get_events(&client, ctx, params).await,
            GetNonce(params) => StarknetRequest::get_nonce(&client, ctx, params).await,
            L1ToL2Messages(params) => {
                StarknetRequest::l1_to_l2_messages(&client, ctx, params).await
            }
            L1ToL2MessageNonce => StarknetRequest::l1_to_l2_message_nonce(&client, ctx).await,
            L1ToL2MessageCancellations(params) => {
                StarknetRequest::l1_to_l2_message_cancellations(&client, ctx, params).await
            }
            L2ToL1Messages(params) => {
                StarknetRequest::l2_to_l1_messages(&client, ctx, params).await
            }
            AddDeclareTransaction(params) => {
                StarknetRequest::add_declare_transaction(&client, ctx, params).await
            }
            AddDeployAccountTransaction(params) => {
                StarknetRequest::add_deploy_account_transaction(&client, ctx, params).await
            }
            GetContractStorageProof(params) => {
                StarknetRequest::get_contract_storage_proof(&client, ctx, params).await
            }
            AddInvokeTransaction(params) => {
                StarknetRequest::add_invoke_transaction(&client, ctx, params).await
            }
        }
    }
}

impl StarknetRequest {
    async fn get_block_with_tx_hashes(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetBlockWithTxHashes,
    ) -> Result<()> {
        todo!();
    }

    async fn get_block_with_txs(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetBlockWithTxs,
    ) -> Result<()> {
        todo!();
    }

    async fn get_state_update(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetStateUpdate,
    ) -> Result<()> {
        todo!();
    }

    async fn get_storage_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetStorageAt,
    ) -> Result<()> {
        todo!();
    }

    async fn get_transaction_by_hash(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetTransactionByHash,
    ) -> Result<()> {
        todo!();
    }

    async fn get_transaction_by_block_id_and_index(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetTransactionByBlockIdAndIndex,
    ) -> Result<()> {
        todo!();
    }

    async fn get_transaction_receipt(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetTransactionReceipt,
    ) -> Result<()> {
        todo!();
    }

    async fn get_class(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetClass,
    ) -> Result<()> {
        todo!();
    }

    async fn get_class_hash_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetClassHashAt,
    ) -> Result<()> {
        todo!();
    }

    async fn get_class_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetClassAt,
    ) -> Result<()> {
        todo!();
    }

    async fn get_block_transaction_count(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetBlockTransactionCount,
    ) -> Result<()> {
        todo!();
    }

    async fn get_call(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetCall,
    ) -> Result<()> {
        todo!();
    }

    async fn estimate_fee(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetEstimateFee,
    ) -> Result<()> {
        todo!();
    }

    async fn block_number(client: &BeerusLightClient, mut ctx: TaskContext) -> Result<()> {
        let block_number = client.starknet_lightclient.block_number().await?;

        ctx.run_on_main_thread(move |ctx| {
            // Insert into entity with LightClientRequest
            ctx.world.spawn((Starknet, BlockNumber::new(block_number)));
        })
        .await;

        Ok(())
    }

    async fn block_hash_and_number(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    async fn chain_id(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    async fn pending_transactions(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    async fn syncing(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    async fn get_events(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetEvents,
    ) -> Result<()> {
        todo!();
    }

    async fn get_nonce(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetNonce,
    ) -> Result<()> {
        todo!();
    }

    async fn l1_to_l2_messages(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetL1ToL2Messages,
    ) -> Result<()> {
        todo!();
    }

    async fn l1_to_l2_message_nonce(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    async fn l1_to_l2_message_cancellations(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetL1ToL2MessageCancellations,
    ) -> Result<()> {
        todo!();
    }

    async fn l2_to_l1_messages(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetL2ToL1Messages,
    ) -> Result<()> {
        todo!();
    }

    async fn add_declare_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetAddDeclareTransaction,
    ) -> Result<()> {
        todo!();
    }

    async fn add_deploy_account_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetAddDeployAccountTransaction,
    ) -> Result<()> {
        todo!();
    }

    async fn get_contract_storage_proof(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetGetContractStorageProof,
    ) -> Result<()> {
        todo!();
    }

    async fn add_invoke_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &StarknetAddInvokeTransaction,
    ) -> Result<()> {
        todo!();
    }
}

////////////////////////////////////////////////////////////////////////
// Components
////////////////////////////////////////////////////////////////////////

/// Labeling component for Starknet related entity
#[derive(Component)]
pub struct Starknet;
