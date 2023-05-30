use async_trait::async_trait;
use beerus_core::lightclient::beerus::BeerusLightClient;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::system::IntoPipeSystem;
use bevy_tokio_tasks::TaskContext;
use dojo_macros::LightClientSystem;
use ethabi::ethereum_types::H256;
use ethabi::Address;
use ethers::types::Filter;
use eyre::Result;
use helios::types::CallOpts;
use starknet::core::types::BlockTag;

use crate::light_client::{handle_request_error, BlockNumber, RequestSender};

pub struct EthereumClientPlugin;

impl Plugin for EthereumClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EthereumGetBalance>()
            .add_event::<EthereumGetTransactionCount>()
            .add_event::<EthereumGetCode>()
            .add_event::<EthereumCall>()
            .add_event::<EthereumEstimateGas>()
            .add_event::<EthereumGetChainId>()
            .add_event::<EthereumGasPrice>()
            .add_event::<EthereumMaxPriorityFeePerGas>()
            .add_event::<EthereumBlockNumber>()
            .add_event::<EthereumGetBlockByNumber>()
            .add_event::<EthereumGetBlockByHash>()
            .add_event::<EthereumSendRawTransaction>()
            .add_event::<EthereumGetTransactionReceipt>()
            .add_event::<EthereumGetLogs>()
            .add_event::<EthereumGetStorageAt>()
            .add_event::<EthereumGetBlockTransactionCountByHash>()
            .add_event::<EthereumGetBlockTransactionCountByNumber>()
            .add_event::<EthereumCoinbase>()
            .add_event::<EthereumSyncing>()
            .add_event::<EthereumGetTransactionByHash>()
            .add_systems((
                get_balance.pipe(handle_request_error),
                get_transaction_count.pipe(handle_request_error),
                get_code.pipe(handle_request_error),
                call.pipe(handle_request_error),
                estimate_gas.pipe(handle_request_error),
                get_chain_id.pipe(handle_request_error),
                gas_price.pipe(handle_request_error),
                max_priority_fee_per_gas.pipe(handle_request_error),
                block_number.pipe(handle_request_error),
                get_block_by_hash.pipe(handle_request_error),
                send_raw_transaction.pipe(handle_request_error),
                get_logs.pipe(handle_request_error),
                get_storage_at.pipe(handle_request_error),
                get_block_transaction_count_by_hash.pipe(handle_request_error),
                get_block_transaction_count_by_number.pipe(handle_request_error),
            ))
            .add_systems((
                coinbase.pipe(handle_request_error),
                syncing.pipe(handle_request_error),
                get_transaction_by_hash.pipe(handle_request_error),
                get_transaction_by_block_hash_and_index.pipe(handle_request_error),
            ));
    }
}

////////////////////////////////////////////////////////////////////////
// Events
////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetBalance {
    pub address: Address,
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetTransactionCount {
    pub address: Address,
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetCode {
    pub address: Address,
    pub block: BlockTag,
}

#[derive(Clone, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthereumCall {
    pub opts: CallOpts,
    pub block: BlockTag,
}

#[derive(Clone, Debug, LightClientSystem)]
pub struct EthereumEstimateGas {
    pub opts: CallOpts,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetChainId;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGasPrice;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumMaxPriorityFeePerGas;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumBlockNumber;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetBlockByNumber {
    pub block: BlockTag,
    pub full_tx: bool,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetBlockByHash {
    pub hash: &'static str,
    pub full_tx: bool,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthereumSendRawTransaction {
    pub bytes: &'static str,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetTransactionReceipt {
    pub tx_hash: &'static str,
}

#[derive(Clone, Debug, LightClientSystem)]
pub struct EthereumGetLogs {
    pub filter: Filter,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthereumGetStorageAt {
    pub address: Address,
    pub slot: H256,
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
/// Not supported: https://github.com/keep-starknet-strange/beerus#endpoint-support
pub struct EthereumGetBlockTransactionCountByHash {
    pub hash: &'static str,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetBlockTransactionCountByNumber {
    pub block: BlockTag,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumCoinbase;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumSyncing;

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetTransactionByHash {
    pub tx_hash: &'static str,
}

#[derive(Clone, Copy, Debug, LightClientSystem)]
pub struct EthereumGetTransactionByBlockHashAndIndex {
    pub hash: &'static str,
    pub index: usize,
}

#[derive(Debug)]
pub enum EthereumRequest {
    GetBalance(EthereumGetBalance),
    GetTransactionCount(EthereumGetTransactionCount),
    GetCode(EthereumGetCode),
    Call(EthereumCall),
    EstimateGas(EthereumEstimateGas),
    GetChainId,
    GasPrice,
    MaxPriorityFeePerGas,
    BlockNumber,
    GetBlockByNumber(EthereumGetBlockByNumber),
    GetBlockByHash(EthereumGetBlockByHash),
    SendRawTransaction(EthereumSendRawTransaction),
    GetTransactionReceipt(EthereumGetTransactionReceipt),
    GetLogs(EthereumGetLogs),
    GetStorageAt(EthereumGetStorageAt),
    GetBlockTransactionCountByHash(EthereumGetBlockTransactionCountByHash),
    GetBlockTransactionCountByNumber(EthereumGetBlockTransactionCountByNumber),
    Coinbase,
    Syncing,
    GetTransactionByHash(EthereumGetTransactionByHash),
    GetTransactionByBlockHashAndIndex(EthereumGetTransactionByBlockHashAndIndex),
}

#[async_trait]
impl RequestSender for EthereumRequest {
    async fn send(&self, client: &BeerusLightClient, ctx: TaskContext) -> Result<()> {
        use EthereumRequest::*;

        match self {
            GetBalance(params) => Self::get_balance(&client, ctx, &params).await,
            GetTransactionCount(params) => Self::get_transaction_count(&client, ctx, &params).await,
            GetCode(params) => Self::get_code(&client, ctx, &params).await,
            Call(params) => Self::call(&client, ctx, &params).await,
            EstimateGas(params) => Self::estimate_gas(&client, ctx, &params).await,
            GetChainId => Self::get_chain_id(&client, ctx).await,
            GasPrice => Self::get_price(&client, ctx).await,
            MaxPriorityFeePerGas => Self::max_priority_fee_per_gas(&client, ctx).await,
            BlockNumber => Self::block_number(&client, ctx).await,
            GetBlockByNumber(params) => Self::get_block_by_number(&client, ctx, &params).await,
            GetBlockByHash(params) => Self::get_block_by_hash(&client, ctx, &params).await,
            SendRawTransaction(params) => Self::send_raw_transaction(&client, ctx, &params).await,
            GetTransactionReceipt(params) => {
                Self::get_transaction_receipt(&client, ctx, &params).await
            }
            GetLogs(params) => Self::get_logs(&client, ctx, &params).await,
            GetStorageAt(params) => Self::get_storage_at(&client, ctx, &params).await,
            GetBlockTransactionCountByHash(params) => {
                Self::get_block_transaction_count_by_hash(&client, ctx, &params).await
            }
            GetBlockTransactionCountByNumber(params) => {
                Self::get_block_transaction_count_by_number(&client, ctx, &params).await
            }
            Coinbase => Self::coinbase(&client, ctx).await,
            Syncing => Self::syncing(&client, ctx).await,
            GetTransactionByHash(params) => {
                Self::get_transaction_by_hash(&client, ctx, &params).await
            }
            GetTransactionByBlockHashAndIndex(params) => {
                Self::get_transaction_by_block_hash_and_index(&client, ctx, &params).await
            }
        }
    }
}

impl EthereumRequest {
    async fn get_balance(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _param: &EthereumGetBalance,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_transaction_count(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetTransactionCount,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_code(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetCode,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn call(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumCall,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn estimate_gas(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumEstimateGas,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_chain_id(_client: &BeerusLightClient, mut _ctx: TaskContext) -> eyre::Result<()> {
        todo!();
    }

    async fn get_price(_client: &BeerusLightClient, mut _ctx: TaskContext) -> eyre::Result<()> {
        todo!();
    }

    async fn max_priority_fee_per_gas(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn block_number(client: &BeerusLightClient, mut ctx: TaskContext) -> eyre::Result<()> {
        let block_number = client.ethereum_lightclient.lock().await.get_block_number().await?;

        ctx.run_on_main_thread(move |ctx| {
            ctx.world.spawn((Ethereum, BlockNumber::new(block_number)));
        })
        .await;

        Ok(())
    }

    async fn get_block_by_number(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetBlockByNumber,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_block_by_hash(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetBlockByHash,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn send_raw_transaction(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumSendRawTransaction,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_transaction_receipt(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetTransactionReceipt,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_logs(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetLogs,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_storage_at(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetStorageAt,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_block_transaction_count_by_hash(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetBlockTransactionCountByHash,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_block_transaction_count_by_number(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetBlockTransactionCountByNumber,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn coinbase(_client: &BeerusLightClient, mut _ctx: TaskContext) -> eyre::Result<()> {
        todo!();
    }

    async fn syncing(_client: &BeerusLightClient, mut _ctx: TaskContext) -> eyre::Result<()> {
        todo!();
    }

    async fn get_transaction_by_hash(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetTransactionByHash,
    ) -> eyre::Result<()> {
        todo!();
    }

    async fn get_transaction_by_block_hash_and_index(
        _client: &BeerusLightClient,
        mut _ctx: TaskContext,
        _params: &EthereumGetTransactionByBlockHashAndIndex,
    ) -> eyre::Result<()> {
        todo!();
    }
}

////////////////////////////////////////////////////////////////////////
// Components
////////////////////////////////////////////////////////////////////////

/// Labeling component for Ethereum related entity
#[derive(Component)]
pub struct Ethereum;
