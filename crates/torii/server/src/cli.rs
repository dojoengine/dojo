use std::env;
use std::str::FromStr;

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::Parser;
use dojo_world::manifest::Manifest;
use dojo_world::metadata::{dojo_metadata_from_workspace, Environment};
use scarb::core::Config;
use sqlx::sqlite::SqlitePoolOptions;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio_util::sync::CancellationToken;
use torii_core::processors::register_component::RegisterComponentProcessor;
use torii_core::processors::register_system::RegisterSystemProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::sql::Sql;
use torii_core::State;
use torii_graphql::server::start_graphql;
use tracing::error;
use tracing_subscriber::fmt;
use url::Url;

use crate::engine::Processors;
use crate::indexer::Indexer;

mod engine;
mod indexer;

/// Dojo World Indexer
#[derive(Parser, Debug)]
#[command(name = "torii", author, version, about, long_about = None)]
struct Args {
    /// The world to index
    #[arg(short, long = "world")]
    world_address: Option<FieldElement>,
    /// The rpc endpoint to use
    #[arg(long, default_value = "http://localhost:5050")]
    rpc: String,
    /// Database url
    #[arg(short, long, default_value = "sqlite::memory:")]
    database_url: String,
    /// Specify a local manifest to intiailize from
    #[arg(short, long)]
    manifest: Option<Utf8PathBuf>,
    /// Specify a block to start indexing from, ignored if stored head exists
    #[arg(short, long, default_value = "0")]
    start_block: u64,
}

fn get_world_address(
    args: &Args,
    manifest: &Manifest,
    env_metadata: Option<&Environment>,
) -> anyhow::Result<FieldElement> {
    if let Some(address) = args.world_address {
        Ok(address)
    } else if let Some(address) = manifest.world.address {
        Ok(address)
    } else if let Some(world_address) = env_metadata
        .and_then(|env| env.world_address())
        .or(std::env::var("DOJO_WORLD_ADDRESS").ok().as_deref())
    {
        Ok(FieldElement::from_str(world_address)?)
    } else {
        Err(anyhow!(
            "Could not find World address. Please specify it with --world, or in manifest.json or 
             [tool.dojo.env] in Scarb.toml"
        ))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let subscriber = fmt::Subscriber::builder()
        .with_max_level(tracing::Level::INFO) // Set the maximum log level
        .finish();

    // Set the global subscriber
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set the global tracing subscriber");

    // Setup cancellation for graceful shutdown
    let cts = CancellationToken::new();
    ctrlc::set_handler({
        let cts: CancellationToken = cts.clone();
        move || {
            cts.cancel();
        }
    })?;

    let database_url = &args.database_url;
    #[cfg(feature = "sqlite")]
    let pool = SqlitePoolOptions::new().max_connections(5).connect(database_url).await?;
    sqlx::migrate!("../migrations").run(&pool).await?;

    let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(&args.rpc).unwrap()));

    // Load manifest
    let manifest_path = scarb::ops::find_manifest_path(None)?;
    let config = Config::builder(manifest_path.clone())
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()?;
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config)?;
    let target_dir = ws.target_dir().path_existent()?;
    let target_dir = target_dir.join(ws.config().profile().as_str());
    let manifest = Manifest::load_from_path(target_dir.join("manifest.json"))?;

    // Get world address
    let world_address = get_world_address(
        &args,
        &manifest,
        dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned()).as_ref(),
    )?;

    let state = Sql::new(pool.clone(), world_address).await?;
    state.load_from_manifest(manifest.clone()).await?;
    let processors = Processors {
        event: vec![
            Box::new(RegisterComponentProcessor),
            Box::new(RegisterSystemProcessor),
            Box::new(StoreSetRecordProcessor),
        ],
        ..Processors::default()
    };

    let indexer =
        Indexer::new(&state, &provider, processors, manifest, world_address, args.start_block);
    let graphql = start_graphql(&pool);

    tokio::select! {
        res = indexer.start() => {
            if let Err(e) = res {
                error!("Indexer failed with error: {:?}", e);
            }
        }
        res = graphql => {
            if let Err(e) = res {
                error!("GraphQL server failed with error: {:?}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down");
        }
    }

    Ok(())
}
