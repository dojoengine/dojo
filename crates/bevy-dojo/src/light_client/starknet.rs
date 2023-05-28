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

use crate::light_client::{handle_errors, BlockNumber, LightClient, LightClientRequest};

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
            .add_systems((
                get_block_with_tx_hashes.pipe(handle_errors),
                block_number.pipe(handle_errors),
            ));
    }
}

////////////////////////////////////////////////////////////////////////
// Events
////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct StarknetGetBlockWithTxHashes {
    pub block_id: BlockId,
}

#[derive(Debug)]
pub struct StarknetGetBlockWithTxs {
    pub block_id: BlockId,
}

#[derive(Debug)]
pub struct StarknetGetStateUpdate {
    pub block_id: BlockId,
}

#[derive(Debug)]
pub struct StarknetGetStorageAt {
    pub address: FieldElement,
    pub key: FieldElement,
    pub block_number: u64,
}

#[derive(Debug)]
pub struct StarknetGetTransactionByHash {
    pub hash: FieldElement,
}

#[derive(Debug)]
pub struct StarknetGetTransactionByBlockIdAndIndex {
    pub block_id: BlockId,
    pub index: u64,
}

#[derive(Debug)]
pub struct StarknetGetTransactionReceipt {
    pub hash: FieldElement,
}

#[derive(Debug)]
pub struct StarknetGetClass {
    pub block_id: BlockId,
    pub class_hash: FieldElement,
}

#[derive(Debug)]
pub struct StarknetGetClassHashAt {
    pub block_id: BlockId,
    pub contract_address: FieldElement,
}

#[derive(Debug)]
pub struct StarknetGetClassAt {
    pub block_id: BlockId,
    pub contract_address: FieldElement,
}

#[derive(Debug)]
pub struct StarknetGetBlockTransactionCount {
    pub block_id: BlockId,
}

#[derive(Debug)]
pub struct StarknetCall {
    pub opts: FunctionCall,
    pub block_number: u64,
}

#[derive(Debug)]
pub struct StarknetEstimateFee {
    pub tx: BroadcastedTransaction,
    pub block_id: BlockId,
}

#[derive(Debug)]
pub struct StarknetBlockNumber;

#[derive(Debug)]
pub struct StarknetBlockHashAndNumber;

#[derive(Debug)]
pub struct StarknetChainId;

#[derive(Debug)]
pub struct StarknetPendingTransactions;

#[derive(Debug)]
pub struct StarknetSyncing;

/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
#[derive(Debug)]
pub struct StarknetGetEvents {
    pub filter: EventFilter,
    pub continuation_token: Option<String>,
    pub chunk_size: u64,
}

#[derive(Debug)]
pub struct StarknetGetNonce {
    pub block_id: BlockId,
    pub address: FieldElement,
}

#[derive(Debug)]
pub struct StarknetL1ToL2Messages {
    pub msg_hash: U256,
}

#[derive(Debug)]
pub struct StarknetL1ToL2MessageNonce;

#[derive(Debug)]
pub struct StarknetL1ToL2MessageCancellations {
    pub msg_hash: U256,
}

#[derive(Debug)]
pub struct StarknetL2ToL1Messages {
    pub msg_hash: U256,
}

#[derive(Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddDeclareTransaction {
    pub declare_transaction: BroadcastedDeclareTransaction,
}

#[derive(Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetAddDeployAccountTransaction {
    pub contract_class: String,
    pub version: String,
    pub contract_address_salt: String,
    pub constructor_calldata: Vec<String>,
}

#[derive(Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct StarknetGetContractStorageProof {
    pub block_id: BlockId,
    pub contract_address: String,
    pub keys: Vec<String>,
}

#[derive(Debug)]
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
    events.iter().try_for_each(|e| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_block_with_tx_hashes(*e))?;

        Ok(())
    })
}

/// Send [StarknetGetBlockWithTxs] request to remote node.
fn get_block_with_txs(
    mut events: EventReader<StarknetGetBlockWithTxs>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|e| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_get_block_with_txs(*e))?;

        Ok(())
    })
}

/// Send [StarknetBlockNumber] request to remote node.
fn block_number(
    mut events: EventReader<StarknetBlockNumber>,
    query: Query<&LightClient>,
) -> Result<()> {
    events.iter().try_for_each(|e| {
        let client = query.get_single()?;
        client.send(LightClientRequest::starknet_block_number(*e))?;

        Ok(())
    })
}

////////////////////////////////////////////////////////////////////////
// Utils
////////////////////////////////////////////////////////////////////////

use StarknetRequest::*;
impl LightClientRequest {
    pub fn starknet_get_block_with_tx_hashes(event: StarknetGetBlockWithTxHashes) -> Self {
        Self::Starknet(GetBlockWithTxHashes(event))
    }

    pub fn starknet_get_block_with_txs(event: StarknetGetBlockWithTxs) -> Self {
        Self::Starknet(GetBlockWithTxs(event))
    }

    pub fn starknet_get_state_update(event: StarknetGetStateUpdate) -> Self {
        Self::Starknet(GetStateUpdate(event))
    }

    pub fn starknet_get_storage_at(event: StarknetGetStorageAt) -> Self {
        Self::Starknet(GetStorageAt(event))
    }

    pub fn starknet_get_transaction_by_hash(event: StarknetGetTransactionByHash) -> Self {
        Self::Starknet(GetTransactionByHash(event))
    }

    pub fn starknet_get_transaction_by_block_id_and_index(
        event: StarknetGetTransactionByBlockIdAndIndex,
    ) -> Self {
        Self::Starknet(GetTransactionByBlockIdAndIndex(event))
    }

    pub fn starknet_get_transaction_receipt(event: StarknetGetTransactionReceipt) -> Self {
        Self::Starknet(GetTransactionReceipt(event))
    }

    pub fn starknet_get_class(event: StarknetGetClass) -> Self {
        Self::Starknet(GetClass(event))
    }

    pub fn starknet_get_class_hash_at(event: StarknetGetClassHashAt) -> Self {
        Self::Starknet(GetClassHashAt(event))
    }

    pub fn starknet_get_class_at(event: StarknetGetClassAt) -> Self {
        Self::Starknet(GetClassAt(event))
    }

    pub fn starknet_get_block_transaction_count(event: StarknetGetBlockTransactionCount) -> Self {
        Self::Starknet(GetBlockTransactionCount(event))
    }

    pub fn starknet_call(event: StarknetCall) -> Self {
        Self::Starknet(Call(event))
    }

    pub fn starknet_estimate_fee(event: StarknetEstimateFee) -> Self {
        Self::Starknet(EstimateFee(event))
    }

    pub fn starknet_block_number(event: StarknetBlockNumber) -> Self {
        Self::Starknet(BlockNumber(event))
    }

    pub fn starknet_block_hash_and_number(event: StarknetBlockHashAndNumber) -> Self {
        Self::Starknet(BlockHashAndNumber(event))
    }

    pub fn starknet_chain_id(event: StarknetChainId) -> Self {
        Self::Starknet(ChainId(event))
    }

    pub fn starknet_syncing(event: StarknetSyncing) -> Self {
        Self::Starknet(Syncing(event))
    }

    pub fn starknet_get_events(event: StarknetGetEvents) -> Self {
        Self::Starknet(GetEvents(event))
    }

    pub fn starknet_get_nonce(event: StarknetGetNonce) -> Self {
        Self::Starknet(GetNonce(event))
    }

    pub fn starknet_l1_to_l2_messages(event: StarknetL1ToL2Messages) -> Self {
        Self::Starknet(L1ToL2Messages(event))
    }

    pub fn starknet_l1_to_l2_message_nonce(event: StarknetL1ToL2MessageNonce) -> Self {
        Self::Starknet(L1ToL2MessageNonce(event))
    }

    pub fn starknet_l1_to_l2_message_cancellations(
        event: StarknetL1ToL2MessageCancellations,
    ) -> Self {
        Self::Starknet(L1ToL2MessageCancellations(event))
    }

    pub fn starknet_l2_to_l1_messages(event: StarknetL2ToL1Messages) -> Self {
        Self::Starknet(L2ToL1Messages(event))
    }

    pub fn starknet_add_declare_transaction(event: StarknetAddDeclareTransaction) -> Self {
        Self::Starknet(AddDeclareTransaction(event))
    }

    pub fn starknet_add_deploy_account_transaction(
        event: StarknetAddDeployAccountTransaction,
    ) -> Self {
        Self::Starknet(AddDeployAccountTransaction(event))
    }

    pub fn starknet_get_contract_storage_proof(event: StarknetGetContractStorageProof) -> Self {
        Self::Starknet(GetContractStorageProof(event))
    }

    pub fn starknet_add_invoke_transaction(event: StarknetAddInvokeTransaction) -> Self {
        Self::Starknet(AddInvokeTransaction(event))
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
    BlockNumber(StarknetBlockNumber),
    BlockHashAndNumber(StarknetBlockHashAndNumber),
    ChainId(StarknetChainId),
    PendingTransactions(StarknetPendingTransactions),
    Syncing(StarknetSyncing),
    GetEvents(StarknetGetEvents),
    GetNonce(StarknetGetNonce),
    L1ToL2Messages(StarknetL1ToL2Messages),
    L1ToL2MessageNonce(StarknetL1ToL2MessageNonce),
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
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_block_with_txs(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_state_update(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_storage_at(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn get_transaction_by_hash(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_transaction_by_block_id_and_index(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_transaction_receipt(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_class(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn get_class_hash_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_class_at(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn get_block_transaction_count(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_call(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn estimate_fee(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn block_number(
        client: &BeerusLightClient,
        mut ctx: TaskContext,
        event: StarknetBlockNumber,
    ) -> Result<()> {
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

    pub async fn pendint_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn syncing(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn get_event(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn get_nonce(_client: &BeerusLightClient, mut _ctx: TaskContext) -> Result<()> {
        todo!();
    }

    pub async fn l1_to_l2_message(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
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
    ) -> Result<()> {
        todo!();
    }

    pub async fn l2_to_l1_messages(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn add_declare_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn add_deploy_account_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn get_contract_storage_proof(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }

    pub async fn add_invoke_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> Result<()> {
        todo!();
    }
}

/// Labeling component for Starknet related entity
#[derive(Component)]
pub struct Starknet;
