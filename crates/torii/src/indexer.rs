use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use num::BigUint;
use starknet::core::types::{
    BlockId, BlockWithTxs, Event, InvokeTransaction, MaybePendingBlockWithTxs,
    MaybePendingTransactionReceipt, StarknetError, Transaction, TransactionReceipt,
};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet::providers::{Provider, ProviderError};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

// use crate::processors::component_register::ComponentRegistrationProcessor;
// use crate::processors::component_state_update::ComponentStateUpdateProcessor;
// use crate::processors::system_register::SystemRegistrationProcessor;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};
use crate::storage::Storage;

pub async fn start_indexer<S: Storage, T: JsonRpcTransport + Sync + Send>(
    _ct: CancellationToken,
    _world: BigUint,
    storage: &S,
    provider: &JsonRpcClient<T>,
) -> Result<(), Box<dyn Error>> {
    info!("starting indexer");

    let block_processors: Vec<Arc<dyn BlockProcessor<S, T>>> = vec![];
    let transaction_processors: Vec<Arc<dyn TransactionProcessor<S, T>>> = vec![];
    let event_processors: Vec<Arc<dyn EventProcessor<S, T>>> = vec![];

    let mut current_block_number = storage.head().await?;

    loop {
        sleep(Duration::from_secs(1)).await;

        let block_with_txs =
            match provider.get_block_with_txs(BlockId::Number(current_block_number)).await {
                Ok(block_with_txs) => block_with_txs,
                Err(e) => {
                    if let ProviderError::StarknetError(StarknetError::BlockNotFound) = e {
                        continue;
                    }

                    error!("getting  block: {}", e);
                    continue;
                }
            };

        let block_with_txs = match block_with_txs {
            MaybePendingBlockWithTxs::Block(block_with_txs) => block_with_txs,
            _ => continue,
        };

        process_block(storage, provider, &block_processors, &block_with_txs).await?;

        for transaction in block_with_txs.transactions {
            let invoke_transaction = match &transaction {
                Transaction::Invoke(invoke_transaction) => invoke_transaction,
                _ => continue,
            };

            let invoke_transaction = match invoke_transaction {
                InvokeTransaction::V1(invoke_transaction) => invoke_transaction,
                _ => continue,
            };

            let receipt =
                match provider.get_transaction_receipt(invoke_transaction.transaction_hash).await {
                    Ok(receipt) => receipt,
                    _ => continue,
                };

            let receipt = match receipt {
                MaybePendingTransactionReceipt::Receipt(receipt) => receipt,
                _ => continue,
            };

            process_transaction(storage, provider, &transaction_processors, &receipt.clone())
                .await?;

            if let TransactionReceipt::Invoke(invoke_receipt) = receipt.clone() {
                for event in &invoke_receipt.events {
                    process_event(storage, provider, &event_processors, &receipt, event).await?;
                }
            }
        }

        current_block_number += 1;
    }
}

async fn process_block<S: Storage, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn BlockProcessor<S, T>>],
    block: &BlockWithTxs,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(storage, provider, block).await?;
    }
    Ok(())
}

async fn process_transaction<S: Storage, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn TransactionProcessor<S, T>>],
    receipt: &TransactionReceipt,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(storage, provider, receipt).await?;
    }

    Ok(())
}

async fn process_event<S: Storage, T: starknet::providers::jsonrpc::JsonRpcTransport>(
    storage: &S,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn EventProcessor<S, T>>],
    _receipt: &TransactionReceipt,
    event: &Event,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(storage, provider, event).await?;
    }

    Ok(())
}

// #[test]
// fn test_indexer() {
//     use crate::start_apibara;

//     let rpc_url = "http://localhost:5050";
//     let (sequencer, rpc) = build_mock_rpc(5050);
//     let ct = CancellationToken::new();
//     let pool = sqlx::sqlite::SqlitePool::connect("sqlite::memory:").unwrap();
//     let world = BigUint::from(0x1234567890);
//     let provider = JsonRpcClient::new(HttpTransport::new(Uri::parse(rpc_url)));

//     start_indexer(ct, world, Uri::from_str("http://localhost:7171").unwrap(), pool, &provider)
// }
