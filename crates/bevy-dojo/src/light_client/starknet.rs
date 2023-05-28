use beerus_core::lightclient::beerus::BeerusLightClient;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::event::EventReader;
use bevy::ecs::system::{IntoPipeSystem, Query};
use bevy_tokio_tasks::TaskContext;
use ethabi::Uint as U256;
use eyre::Result;
use starknet::core::types::{
    BlockId, BroadcastedDeclareTransaction, BroadcastedInvokeTransaction, BroadcastedTransaction,
    EventFilter, FieldElement, FunctionCall,
};

use crate::light_client::{handle_request_error, BlockNumber, LightClient, LightClientRequest};

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

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetBlockWithTxHashes {
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetBlockWithTxs {
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetStateUpdate {
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetStorageAt {
    pub address: FieldElement,
    pub key: FieldElement,
    pub block_number: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetTransactionByHash {
    pub hash: FieldElement,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetTransactionByBlockIdAndIndex {
    pub block_id: BlockId,
    pub index: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetTransactionReceipt {
    pub hash: FieldElement,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetClass {
    pub block_id: BlockId,
    pub class_hash: FieldElement,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetClassHashAt {
    pub block_id: BlockId,
    pub contract_address: FieldElement,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetClassAt {
    pub block_id: BlockId,
    pub contract_address: FieldElement,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetBlockTransactionCount {
    pub block_id: BlockId,
}

#[derive(Clone, Debug)]
pub struct StarknetCall {
    pub opts: FunctionCall,
    pub block_number: u64,
}

#[derive(Clone, Debug)]
pub struct StarknetEstimateFee {
    pub tx: BroadcastedTransaction,
    pub block_id: BlockId,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetBlockNumber;

#[derive(Clone, Copy, Debug)]
pub struct StarknetBlockHashAndNumber;

#[derive(Clone, Copy, Debug)]
pub struct StarknetChainId;

#[derive(Clone, Copy, Debug)]
pub struct StarknetPendingTransactions;

#[derive(Clone, Copy, Debug)]
pub struct StarknetSyncing;

#[derive(Clone, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetGetEvents {
    pub filter: EventFilter,
    pub continuation_token: Option<String>,
    pub chunk_size: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetGetNonce {
    pub block_id: BlockId,
    pub address: FieldElement,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetL1ToL2Messages {
    pub msg_hash: U256,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetL1ToL2MessageNonce;

#[derive(Clone, Copy, Debug)]
pub struct StarknetL1ToL2MessageCancellations {
    pub msg_hash: U256,
}

#[derive(Clone, Copy, Debug)]
pub struct StarknetL2ToL1Messages {
    pub msg_hash: U256,
}

#[derive(Clone, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddDeclareTransaction {
    pub declare_transaction: BroadcastedDeclareTransaction,
}

#[derive(Clone, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddDeployAccountTransaction {
    pub contract_class: String,
    pub version: String,
    pub contract_address_salt: String,
    pub constructor_calldata: Vec<String>,
}

#[derive(Clone, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetGetContractStorageProof {
    pub block_id: BlockId,
    pub contract_address: String,
    pub keys: Vec<String>,
}

#[derive(Clone, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddInvokeTransaction {
    pub invoke_transaction: BroadcastedInvokeTransaction,
}

////////////////////////////////////////////////////////////////////////
// Systems
////////////////////////////////////////////////////////////////////////

/// Send [StarknetGetBlockWithTxHashes] request to remote node.
fn get_block_with_tx_hashes(
    mut events: EventReader<StarknetGetBlockWithTxHashes>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_block_with_tx_hashes(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetBlockWithTxs] request to remote node.
fn get_block_with_txs(
    mut events: EventReader<StarknetGetBlockWithTxs>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_block_with_txs(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetStateUpdate] request to remote node.
fn get_state_update(
    mut events: EventReader<StarknetGetStateUpdate>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_state_update(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetStorageAt] request to remote node.
fn get_storage_at(
    mut events: EventReader<StarknetGetStorageAt>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_storage_at(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetTransactionByHash] request to remote node.
fn get_transaction_by_hash(
    mut events: EventReader<StarknetGetTransactionByHash>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_transaction_by_hash(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetTransactionByBlockIdAndIndex] request to remote node.
fn get_transaction_by_block_id_and_index(
    mut events: EventReader<StarknetGetTransactionByBlockIdAndIndex>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_transaction_by_block_id_and_index(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetTransactionReceipt] request to remote node.
fn get_transaction_receipt(
    mut events: EventReader<StarknetGetTransactionReceipt>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_transaction_receipt(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetClass] request to remote node.
fn get_class(mut events: EventReader<StarknetGetClass>, query: Query<&LightClient>) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_class(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetClassHashAt] request to remote node.
fn get_class_hash_at(
    mut events: EventReader<StarknetGetClassHashAt>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_class_hash_at(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetClassAt] request to remote node.
fn get_class_at(
    mut events: EventReader<StarknetGetClassAt>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_class_at(*params))?;

        Ok(())
    })
}

/// Send [StarknetGetBlockTransactionCount] request to remote node.
fn get_block_transaction_count(
    mut events: EventReader<StarknetGetBlockTransactionCount>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_block_transaction_count(*params))?;

        Ok(())
    })
}

/// Send [StarknetCall] request to remote node.
fn call(mut events: EventReader<StarknetCall>, query: Query<&LightClient>) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_call(params.clone()))?;

        Ok(())
    })
}

/// Send [StarknetEstimateFee] request to remote node.
fn estimate_fee(
    mut events: EventReader<StarknetEstimateFee>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_estimate_fee(params.clone()))?;

        Ok(())
    })
}

/// Send [StarknetBlockNumber] request to remote node.
fn block_number(
    mut events: EventReader<StarknetBlockNumber>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|_| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_block_number())?;

        Ok(())
    })
}

/// Send [StarknetBlockHashAndNumber] request to remote node.
fn block_hash_and_number(
    mut events: EventReader<StarknetBlockHashAndNumber>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|_| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_block_hash_and_number())?;

        Ok(())
    })
}

/// Send [StarknetChainId] request to remote node.
fn chain_id(mut events: EventReader<StarknetChainId>, query: Query<&LightClient>) -> Result<()> {
    events.iter().try_for_each(|_| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_chain_id())?;

        Ok(())
    })
}

/// Send [StarknetSyncing] request to remote node.
fn syncing(mut events: EventReader<StarknetSyncing>, query: Query<&LightClient>) -> Result<()> {
    events.iter().try_for_each(|_| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_syncing())?;

        Ok(())
    })
}

/// Send [StarknetGetEvents] request to remote node.
fn get_events(
    mut events: EventReader<StarknetGetEvents>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_events(params.clone()))?;

        Ok(())
    })
}

/// Send [StarknetGetNonce] request to remote node.
fn get_nonce(mut events: EventReader<StarknetGetNonce>, query: Query<&LightClient>) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_nonce(*params))?;

        Ok(())
    })
}

/// Send [StarknetL1ToL2Messages] request to remote node.
fn l1_to_l2_messages(
    mut events: EventReader<StarknetL1ToL2Messages>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_l1_to_l2_messages(*params))?;

        Ok(())
    })
}

/// Send [StarknetL1ToL2MessageNonce] request to remote node.
fn l1_to_l2_message_nonce(
    mut events: EventReader<StarknetL1ToL2MessageNonce>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|_| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_l1_to_l2_message_nonce())?;

        Ok(())
    })
}

/// Send [StarknetL1ToL2MessageCancellations] request to remote node.
fn l1_to_l2_message_cancellations(
    mut events: EventReader<StarknetL1ToL2MessageCancellations>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_l1_to_l2_message_cancellations(*params))?;

        Ok(())
    })
}

/// Send [StarknetL2ToL1Messages] request to remote node.
fn l2_to_l1_messages(
    mut events: EventReader<StarknetL2ToL1Messages>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_l2_to_l1_messages(*params))?;

        Ok(())
    })
}

/// Send [StarknetAddDeclareTransaction] request to remote node.
fn add_declare_transaction(
    mut events: EventReader<StarknetAddDeclareTransaction>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_add_declare_transaction(params.clone()))?;

        Ok(())
    })
}

/// Send [StarknetAddDeployAccountTransaction] request to remote node.
fn add_deploy_account_transaction(
    mut events: EventReader<StarknetAddDeployAccountTransaction>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_add_deploy_account_transaction(params.clone()))?;

        Ok(())
    })
}

/// Send [StarknetGetContractStorageProof] request to remote node.
fn get_contract_storage_proof(
    mut events: EventReader<StarknetGetContractStorageProof>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_contract_storage_proof(params.clone()))?;

        Ok(())
    })
}

/// Send [StarknetAddInvokeTransaction] request to remote node.
fn add_invoke_transaction(
    mut events: EventReader<StarknetAddInvokeTransaction>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|params| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_add_invoke_transaction(params.clone()))?;

        Ok(())
    })
}

////////////////////////////////////////////////////////////////////////
// Utils
////////////////////////////////////////////////////////////////////////

use StarknetRequest::*;
impl LightClientRequest {
    pub fn starknet_get_block_with_tx_hashes(params: StarknetGetBlockWithTxHashes) -> Self {
        Self::Starknet(GetBlockWithTxHashes(params))
    }

    pub fn starknet_get_block_with_txs(params: StarknetGetBlockWithTxs) -> Self {
        Self::Starknet(GetBlockWithTxs(params))
    }

    pub fn starknet_get_state_update(params: StarknetGetStateUpdate) -> Self {
        Self::Starknet(GetStateUpdate(params))
    }

    pub fn starknet_get_storage_at(params: StarknetGetStorageAt) -> Self {
        Self::Starknet(GetStorageAt(params))
    }

    pub fn starknet_get_transaction_by_hash(params: StarknetGetTransactionByHash) -> Self {
        Self::Starknet(GetTransactionByHash(params))
    }

    pub fn starknet_get_transaction_by_block_id_and_index(
        params: StarknetGetTransactionByBlockIdAndIndex,
    ) -> Self {
        Self::Starknet(GetTransactionByBlockIdAndIndex(params))
    }

    pub fn starknet_get_transaction_receipt(params: StarknetGetTransactionReceipt) -> Self {
        Self::Starknet(GetTransactionReceipt(params))
    }

    pub fn starknet_get_class(params: StarknetGetClass) -> Self {
        Self::Starknet(GetClass(params))
    }

    pub fn starknet_get_class_hash_at(params: StarknetGetClassHashAt) -> Self {
        Self::Starknet(GetClassHashAt(params))
    }

    pub fn starknet_get_class_at(params: StarknetGetClassAt) -> Self {
        Self::Starknet(GetClassAt(params))
    }

    pub fn starknet_get_block_transaction_count(params: StarknetGetBlockTransactionCount) -> Self {
        Self::Starknet(GetBlockTransactionCount(params))
    }

    pub fn starknet_call(params: StarknetCall) -> Self {
        Self::Starknet(Call(params))
    }

    pub fn starknet_estimate_fee(params: StarknetEstimateFee) -> Self {
        Self::Starknet(EstimateFee(params))
    }

    pub fn starknet_block_number() -> Self {
        Self::Starknet(BlockNumber)
    }

    pub fn starknet_block_hash_and_number() -> Self {
        Self::Starknet(BlockHashAndNumber)
    }

    pub fn starknet_chain_id() -> Self {
        Self::Starknet(ChainId)
    }

    pub fn starknet_syncing() -> Self {
        Self::Starknet(Syncing)
    }

    pub fn starknet_get_events(params: StarknetGetEvents) -> Self {
        Self::Starknet(GetEvents(params))
    }

    pub fn starknet_get_nonce(params: StarknetGetNonce) -> Self {
        Self::Starknet(GetNonce(params))
    }

    pub fn starknet_l1_to_l2_messages(params: StarknetL1ToL2Messages) -> Self {
        Self::Starknet(L1ToL2Messages(params))
    }

    pub fn starknet_l1_to_l2_message_nonce() -> Self {
        Self::Starknet(L1ToL2MessageNonce)
    }

    pub fn starknet_l1_to_l2_message_cancellations(
        params: StarknetL1ToL2MessageCancellations,
    ) -> Self {
        Self::Starknet(L1ToL2MessageCancellations(params))
    }

    pub fn starknet_l2_to_l1_messages(params: StarknetL2ToL1Messages) -> Self {
        Self::Starknet(L2ToL1Messages(params))
    }

    pub fn starknet_add_declare_transaction(params: StarknetAddDeclareTransaction) -> Self {
        Self::Starknet(AddDeclareTransaction(params))
    }

    pub fn starknet_add_deploy_account_transaction(
        params: StarknetAddDeployAccountTransaction,
    ) -> Self {
        Self::Starknet(AddDeployAccountTransaction(params))
    }

    pub fn starknet_get_contract_storage_proof(params: StarknetGetContractStorageProof) -> Self {
        Self::Starknet(GetContractStorageProof(params))
    }

    pub fn starknet_add_invoke_transaction(params: StarknetAddInvokeTransaction) -> Self {
        Self::Starknet(AddInvokeTransaction(params))
    }
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

impl StarknetRequest {
    pub async fn get_block_with_tx_hashes(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetBlockWithTxHashes,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_block_with_txs(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetBlockWithTxs,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_state_update(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetStateUpdate,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_storage_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetStorageAt,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_transaction_by_hash(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetTransactionByHash,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_transaction_by_block_id_and_index(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetTransactionByBlockIdAndIndex,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_transaction_receipt(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetTransactionReceipt,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_class(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetClass,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_class_hash_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetClassHashAt,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_class_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetClassAt,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_block_transaction_count(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetBlockTransactionCount,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_call(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetCall,
    ) -> Result<()> {
        todo!();
    }

    pub async fn estimate_fee(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetEstimateFee,
    ) -> Result<()> {
        todo!();
    }

    pub async fn block_number(client: &BeerusLightClient, mut ctx: TaskContext) -> Result<()> {
        let block_number = client.starknet_lightclient.block_number().await?;

        ctx.run_on_main_thread(move |ctx| {
            // Insert into entity with LightClientRequest
            ctx.world.spawn((Starknet, BlockNumber::new(block_number)));
        })
        .await;

        Ok(())
    }

    pub async fn block_hash_and_number(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn chain_id(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn pending_transactions(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn syncing(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn get_events(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetEvents,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_nonce(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetNonce,
    ) -> Result<()> {
        todo!();
    }

    pub async fn l1_to_l2_messages(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetL1ToL2Messages,
    ) -> Result<()> {
        todo!();
    }

    pub async fn l1_to_l2_message_nonce(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn l1_to_l2_message_cancellations(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetL1ToL2MessageCancellations,
    ) -> Result<()> {
        todo!();
    }

    pub async fn l2_to_l1_messages(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetL2ToL1Messages,
    ) -> Result<()> {
        todo!();
    }

    pub async fn add_declare_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetAddDeclareTransaction,
    ) -> Result<()> {
        todo!();
    }

    pub async fn add_deploy_account_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetAddDeployAccountTransaction,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_contract_storage_proof(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetGetContractStorageProof,
    ) -> Result<()> {
        todo!();
    }

    pub async fn add_invoke_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: StarknetAddInvokeTransaction,
    ) -> Result<()> {
        todo!();
    }
}

/// Labeling component for Starknet related entity
#[derive(Component)]
pub struct Starknet;
