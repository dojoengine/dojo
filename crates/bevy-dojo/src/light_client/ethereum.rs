use beerus_core::lightclient::beerus::BeerusLightClient;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::event::EventReader;
use bevy::ecs::system::{IntoPipeSystem, Query};
use bevy_tokio_tasks::TaskContext;
use ethabi::ethereum_types::H256;
use ethabi::Address;
use ethers::types::Filter;
use eyre::Result;
use helios::types::CallOpts;
use starknet::core::types::BlockTag;

use crate::light_client::{handle_request_error, BlockNumber, LightClient, LightClientRequest};

pub struct EthereumClientPlugin;

impl Plugin for EthereumClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EthGetBalance>()
            .add_event::<EthGetTransactionCount>()
            .add_event::<EthGetCode>()
            .add_event::<EthCall>()
            .add_event::<EthEstimateGas>()
            .add_event::<EthGetChainId>()
            .add_event::<EthGasPrice>()
            .add_event::<EthMaxPriorityFeePerGas>()
            .add_event::<EthBlockNumber>()
            .add_event::<EthGetBlockByNumber>()
            .add_event::<EthGetBlockByHash>()
            .add_event::<EthSendRawTransaction>()
            .add_event::<EthGetTransactionReceipt>()
            .add_event::<EthGetLogs>()
            .add_event::<EthGetStorageAt>()
            .add_event::<EthGetBlockTransactionCountByHash>()
            .add_event::<EthGetBlockTransactionCountByNumber>()
            .add_event::<EthCoinbase>()
            .add_event::<EthSyncing>()
            .add_event::<EthGetTransactionByHash>()
            .add_system(block_number.pipe(handle_request_error));
    }
}

////////////////////////////////////////////////////////////////////////
// Events
////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug)]
pub struct EthGetBalance {
    pub address: Address,
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetTransactionCount {
    pub address: Address,
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetCode {
    pub address: Address,
    pub block: BlockTag,
}

#[derive(Clone, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthCall {
    pub opts: CallOpts,
    pub block: BlockTag,
}

#[derive(Clone, Debug)]
pub struct EthEstimateGas {
    pub opts: CallOpts,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetChainId;

#[derive(Clone, Copy, Debug)]
pub struct EthGasPrice;

#[derive(Clone, Copy, Debug)]
pub struct EthMaxPriorityFeePerGas;

#[derive(Clone, Copy, Debug)]
pub struct EthBlockNumber;

#[derive(Clone, Copy, Debug)]
pub struct EthGetBlockByNumber {
    pub block: BlockTag,
    pub full_tx: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetBlockByHash {
    pub hash: &'static str,
    pub full_tx: bool,
}

#[derive(Clone, Copy, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthSendRawTransaction {
    pub bytes: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetTransactionReceipt {
    pub tx_hash: &'static str,
}

#[derive(Clone, Debug)]
pub struct EthGetLogs {
    pub filter: Filter,
}

#[derive(Clone, Copy, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthGetStorageAt {
    pub address: Address,
    pub slot: H256,
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthGetBlockTransactionCountByHash {
    pub hash: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetBlockTransactionCountByNumber {
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug)]
pub struct EthCoinbase;

#[derive(Clone, Copy, Debug)]
pub struct EthSyncing;

#[derive(Clone, Copy, Debug)]
pub struct EthGetTransactionByHash {
    pub tx_hash: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetTransactionByBlockHashAndIndex {
    pub hash: &'static str,
    pub index: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct EthGetBlockNumber;

////////////////////////////////////////////////////////////////////////
// Systems
////////////////////////////////////////////////////////////////////////

/// React to [EthBlockNumber] event
fn block_number(mut events: EventReader<EthBlockNumber>, query: Query<&LightClient>) -> Result<()> {
    events.iter().try_for_each(|_e| {
        let client = query.get_single()?;
        client.send(LightClientRequest::ethereum_block_number())?;

        Ok(())
    })
}

////////////////////////////////////////////////////////////////////////
// Utils
////////////////////////////////////////////////////////////////////////

use EthRequest::*;
impl LightClientRequest {
    pub fn ethereum_get_balance(params: EthGetBalance) -> Self {
        Self::Ethereum(GetBalance(params))
    }

    pub fn ethereum_get_transaction_count(params: EthGetTransactionCount) -> Self {
        Self::Ethereum(GetTransactionCount(params))
    }

    pub fn ethereum_get_get_code(params: EthGetCode) -> Self {
        Self::Ethereum(GetCode(params))
    }

    pub fn ethereum_get_call(params: EthCall) -> Self {
        Self::Ethereum(Call(params))
    }

    pub fn ethereum_estimate_gas(params: EthEstimateGas) -> Self {
        Self::Ethereum(EstimateGas(params))
    }

    pub fn ethereum_get_chain_id() -> Self {
        Self::Ethereum(GetChainId)
    }

    pub fn ethereum_gas_price() -> Self {
        Self::Ethereum(GasPrice)
    }

    pub fn ethereum_max_priority_fee_per_gas() -> Self {
        Self::Ethereum(MaxPriorityFeePerGas)
    }

    pub fn ethereum_ethereum_block_number() -> Self {
        Self::Ethereum(BlockNumber)
    }

    pub fn ethereum_get_block_by_number(params: EthGetBlockByNumber) -> Self {
        Self::Ethereum(GetBlockByNumber(params))
    }

    pub fn ethereum_get_block_by_hash(params: EthGetBlockByHash) -> Self {
        Self::Ethereum(GetBlockByHash(params))
    }

    pub fn ethereum_send_raw_transaction(params: EthSendRawTransaction) -> Self {
        Self::Ethereum(SendRawTransaction(params))
    }

    pub fn ethereum_get_transaction_receipt(params: EthGetTransactionReceipt) -> Self {
        Self::Ethereum(GetTransactionReceipt(params))
    }

    pub fn ethereum_get_logs(params: EthGetLogs) -> Self {
        Self::Ethereum(GetLogs(params))
    }

    pub fn ethereum_get_storage_at(params: EthGetStorageAt) -> Self {
        Self::Ethereum(GetStorageAt(params))
    }

    pub fn ethereum_get_block_transaction_count_by_hash(
        params: EthGetBlockTransactionCountByHash,
    ) -> Self {
        Self::Ethereum(GetBlockTransactionCountByHash(params))
    }

    pub fn ethereum_get_block_transaction_count_by_number(
        params: EthGetBlockTransactionCountByNumber,
    ) -> Self {
        Self::Ethereum(GetBlockTransactionCountByNumber(params))
    }

    pub fn ethereum_coinbase() -> Self {
        Self::Ethereum(Coinbase)
    }

    pub fn ethereum_syncing() -> Self {
        Self::Ethereum(Syncing)
    }

    pub fn ethereum_get_transaction_by_hash(params: EthGetTransactionByHash) -> Self {
        Self::Ethereum(GetTransactionByHash(params))
    }

    pub fn ethereum_get_transaction_by_block_hash_and_index(
        params: EthGetTransactionByBlockHashAndIndex,
    ) -> Self {
        Self::Ethereum(GetTransactionByBlockHashAndIndex(params))
    }
}

#[derive(Debug)]
pub enum EthRequest {
    GetBalance(EthGetBalance),
    GetTransactionCount(EthGetTransactionCount),
    GetCode(EthGetCode),
    Call(EthCall),
    EstimateGas(EthEstimateGas),
    GetChainId,
    GasPrice,
    MaxPriorityFeePerGas,
    BlockNumber,
    GetBlockByNumber(EthGetBlockByNumber),
    GetBlockByHash(EthGetBlockByHash),
    SendRawTransaction(EthSendRawTransaction),
    GetTransactionReceipt(EthGetTransactionReceipt),
    GetLogs(EthGetLogs),
    GetStorageAt(EthGetStorageAt),
    GetBlockTransactionCountByHash(EthGetBlockTransactionCountByHash),
    GetBlockTransactionCountByNumber(EthGetBlockTransactionCountByNumber),
    Coinbase,
    Syncing,
    GetTransactionByHash(EthGetTransactionByHash),
    GetTransactionByBlockHashAndIndex(EthGetTransactionByBlockHashAndIndex),
}

impl EthRequest {
    pub async fn get_block_number(
        client: &BeerusLightClient,
        mut ctx: TaskContext,
    ) -> eyre::Result<()> {
        let block_number = client.ethereum_lightclient.lock().await.get_block_number().await?;

        ctx.run_on_main_thread(move |ctx| {
            ctx.world.spawn((Ethereum, BlockNumber::new(block_number)));
        })
        .await;

        Ok(())
    }
}

/// Labeling component for Ethereum related entity
#[derive(Component)]
pub struct Ethereum;
