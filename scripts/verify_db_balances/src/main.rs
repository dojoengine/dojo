use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use num_traits::ToPrimitive;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use starknet::core::types::{BlockId, Felt, FunctionCall, U256};
use starknet::macros::selector;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, Url};
use tracing::{error, info, Level};

async fn get_balance_from_starknet(
    account_address: &str,
    contract_address: &str,
    contract_type: &str,
    token_id: &str,
    provider: Arc<JsonRpcClient<HttpTransport>>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let account_address = Felt::from_str(account_address).unwrap();
    let contract_address = Felt::from_str(contract_address).unwrap();

    let balance = match contract_type {
        "ERC20" => {
            let balance = provider
                .call(
                    FunctionCall {
                        contract_address,
                        entry_point_selector: selector!("balanceOf"),
                        calldata: vec![account_address],
                    },
                    BlockId::Tag(starknet::core::types::BlockTag::PreConfirmed),
                )
                .await?;

            let balance_low = balance[0].to_u128().unwrap();
            let balance_high = balance[1].to_u128().unwrap();

            let balance = U256::from_words(balance_low, balance_high);
            format!("{:#064x}", balance)
        }
        "ERC721" => {
            let token_id = Felt::from_str(token_id.split(":").nth(1).unwrap()).unwrap();
            let balance = provider
                .call(
                    FunctionCall {
                        contract_address,
                        entry_point_selector: selector!("ownerOf"),
                        // HACK: assumes token_id.high == 0
                        calldata: vec![token_id, Felt::ZERO],
                    },
                    BlockId::Tag(starknet::core::types::BlockTag::PreConfirmed),
                )
                .await?;
            if account_address != balance[0] {
                format!("{:#064x}", U256::from(0u8))
            } else {
                format!("{:#064x}", U256::from(1u8))
            }
        }
        _ => unreachable!(),
    };
    Ok(balance)
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the SQLite database file
    #[arg(short, long)]
    db_path: String,

    /// RPC URL for the Starknet provider
    #[arg(short, long)]
    rpc_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize the logger
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Parse command line arguments
    let args = Args::parse();

    // Use the provided database path
    let pool = SqlitePool::connect(&format!("sqlite:{}", args.db_path)).await?;

    let rows = sqlx::query(
        "
        SELECT b.account_address, b.contract_address, b.balance, c.contract_type, b.token_id
        FROM balances b
        JOIN contracts c ON b.contract_address = c.contract_address
    ",
    )
    .fetch_all(&pool)
    .await?;

    // Create a semaphore to limit concurrent tasks
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10)); // Adjust the number as needed

    let mut handles = Vec::new();

    // print number of balances
    info!("Checking {} balances", rows.len());

    let provider =
        Arc::new(JsonRpcClient::new(HttpTransport::new(Url::parse(&args.rpc_url).unwrap())));

    // IMPROVEMENT: batch multiple balanceOf calls in same rpc call
    for row in rows {
        let account_address: String = row.get("account_address");
        let contract_address: String = row.get("contract_address");
        let db_balance: String = row.get("balance");
        let contract_type: String = row.get("contract_type");
        let token_id: String = row.get("token_id");
        let semaphore_clone = semaphore.clone();
        let provider = provider.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore_clone.acquire().await.unwrap();
            let starknet_balance = get_balance_from_starknet(
                &account_address,
                &contract_address,
                &contract_type,
                &token_id,
                provider,
            )
            .await?;

            if db_balance != starknet_balance {
                error!(
                    "Mismatch for account {} and contract {}: DB balance = {}, Starknet balance = \
                     {}",
                    account_address, contract_address, db_balance, starknet_balance
                );
            } else {
                info!(
                    "Balance matched for account {} and contract {}",
                    account_address, contract_address
                );
            }
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await??;
    }

    info!("Checked all balances");
    Ok(())
}
