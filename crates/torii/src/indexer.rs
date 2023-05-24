use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use num::BigUint;
use sqlx::{Pool, Sqlite};
use starknet::core::types::{
    BlockId, BlockWithTxs, Event, InvokeTransaction, MaybePendingBlockWithTxs,
    MaybePendingTransactionReceipt, Transaction, TransactionReceipt,
};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet::providers::Provider;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::info;

// use crate::processors::component_register::ComponentRegistrationProcessor;
// use crate::processors::component_state_update::ComponentStateUpdateProcessor;
// use crate::processors::system_register::SystemRegistrationProcessor;
use crate::processors::{BlockProcessor, EventProcessor, TransactionProcessor};

pub async fn start_indexer<T: JsonRpcTransport + Sync + Send>(
    _ct: CancellationToken,
    _world: BigUint,
    pool: &Pool<Sqlite>,
    provider: &JsonRpcClient<T>,
) -> Result<(), Box<dyn Error>> {
    info!("starting indexer");

    let block_processors: Vec<Arc<dyn BlockProcessor<T>>> = vec![];
    let transaction_processors: Vec<Arc<dyn TransactionProcessor<T>>> = vec![];
    let event_processors: Vec<Arc<dyn EventProcessor<T>>> = vec![];

    let mut current_block_number: u64 = 0;

    loop {
        let block_with_txs =
            match provider.get_block_with_txs(BlockId::Number(current_block_number)).await {
                Ok(block_with_txs) => block_with_txs,
                Err(e) => {
                    eprintln!("Error while fetching block: {}", e);
                    sleep(Duration::from_secs(60)).await; // If there's an error, wait longer before the next attempt.
                    continue;
                }
            };

        let block_with_txs = match block_with_txs {
            MaybePendingBlockWithTxs::Block(block_with_txs) => block_with_txs,
            _ => continue,
        };

        process_block(pool, provider, &block_processors, &block_with_txs).await?;

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

            process_transaction(pool, provider, &transaction_processors, &receipt.clone()).await?;

            if let TransactionReceipt::Invoke(invoke_receipt) = receipt.clone() {
                for event in &invoke_receipt.events {
                    process_event(pool, provider, &event_processors, &receipt, event).await?;
                }
            }
        }

        current_block_number += 1;
        sleep(Duration::from_secs(15)).await;
    }
}

async fn process_block<T: starknet::providers::jsonrpc::JsonRpcTransport>(
    pool: &Pool<Sqlite>,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn BlockProcessor<T>>],
    block: &BlockWithTxs,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(pool, provider, block).await?;
    }
    Ok(())
}

async fn process_transaction<T: starknet::providers::jsonrpc::JsonRpcTransport>(
    pool: &Pool<Sqlite>,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn TransactionProcessor<T>>],
    receipt: &TransactionReceipt,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(pool, provider, receipt).await?;
    }

    Ok(())
}

async fn process_event<T: starknet::providers::jsonrpc::JsonRpcTransport>(
    pool: &Pool<Sqlite>,
    provider: &JsonRpcClient<T>,
    processors: &[Arc<dyn EventProcessor<T>>],
    _receipt: &TransactionReceipt,
    event: &Event,
) -> Result<(), Box<dyn Error>> {
    for processor in processors {
        processor.process(pool, provider, event).await?;
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
