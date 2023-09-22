use std::env;
use std::str::FromStr;

use anyhow::{anyhow, Context};
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
use torii_client::contract::world::WorldContractReader;
use torii_core::processors::register_component::RegisterComponentProcessor;
use torii_core::processors::register_system::RegisterSystemProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::processors::store_system_call::StoreSystemCallProcessor;
use torii_core::sql::Sql;
use tracing::error;
use tracing_subscriber::fmt;
use url::Url;
use warp::Filter;

use crate::engine::Processors;
use crate::indexer::Indexer;

mod engine;
mod indexer;

/// Dojo World Indexer
#[derive(Parser, Debug)]
#[command(name = "torii", author, version, about, long_about = None)]
struct Args {
    /// The world to index
    #[arg(short, long = "world", env = "DOJO_WORLD_ADDRESS")]
    world_address: Option<FieldElement>,
    /// The rpc endpoint to use
    #[arg(long, default_value = "http://localhost:5050")]
    rpc: String,
    /// Database url
    #[arg(short, long, default_value = "sqlite::memory:")]
    database_url: String,
    /// Specify a local manifest to intiailize from
    #[arg(short, long, env = "DOJO_MANIFEST_FILE")]
    manifest: Option<Utf8PathBuf>,
    /// Specify a block to start indexing from, ignored if stored head exists
    #[arg(short, long, default_value = "0")]
    start_block: u64,
    /// Host address for GraphQL/gRPC endpoints
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    /// Port number for GraphQL endpoint
    #[arg(long, default_value = "8080")]
    graphql_port: u16,
    /// Port number for gRPC endpoint
    #[arg(long, default_value = "50051")]
    grpc_port: u16,
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

    let (manifest, env) = get_manifest_and_env(args.manifest.as_ref())
        .with_context(|| "Failed to get manifest file".to_string())?;

    // Get world address
    let world_address = get_world_address(&args, &manifest, env.as_ref())?;
    let world = WorldContractReader::new(world_address, &provider);

    let db = Sql::new(pool.clone(), world_address).await?;
    db.load_from_manifest(manifest.clone()).await?;
    let processors = Processors {
        event: vec![
            Box::new(RegisterComponentProcessor),
            Box::new(RegisterSystemProcessor),
            Box::new(StoreSetRecordProcessor),
        ],
        transaction: vec![Box::new(StoreSystemCallProcessor)],
        ..Processors::default()
    };

    let indexer =
        Indexer::new(&world, &db, &provider, processors, manifest, world_address, args.start_block);

    let base_route = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({ "success": true })));
    let routes = torii_graphql::route::filter(&pool)
        .await
        .or(torii_grpc::route::filter(&pool))
        .or(base_route);
    let server = warp::serve(routes);
    let server = server.run((args.host.parse::<std::net::IpAddr>()?, args.graphql_port));

    tokio::select! {
        res = indexer.start() => {
            if let Err(e) = res {
                error!("Indexer failed with error: {:?}", e);
            }
        }
        _ = server => {}
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down");
        }
    }

    Ok(())
}

// Tries to find scarb manifest first for env variables
//
// Use manifest path from cli args,
// else uses scarb manifest to derive path of dojo manifest file,
// else try to derive manifest path from scarb manifest
// else try `./target/dev/manifest.json` as dojo manifest path
//
// If neither of this work return an error and exit
fn get_manifest_and_env(
    args_path: Option<&Utf8PathBuf>,
) -> anyhow::Result<(Manifest, Option<Environment>)> {
    let config;
    let ws = if let Ok(scarb_manifest_path) = scarb::ops::find_manifest_path(None) {
        config = Config::builder(scarb_manifest_path)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .build()
            .with_context(|| "Couldn't build scarb config".to_string())?;
        scarb::ops::read_workspace(config.manifest_path(), &config).ok()
    } else {
        None
    };

    let manifest = if let Some(manifest_path) = args_path {
        Manifest::load_from_path(manifest_path)?
    } else if let Some(ref ws) = ws {
        let target_dir = ws.target_dir().path_existent()?;
        let target_dir = target_dir.join(ws.config().profile().as_str());
        let manifest_path = target_dir.join("manifest.json");
        Manifest::load_from_path(manifest_path)?
    } else {
        return Err(anyhow!(
            "Cannot find Scarb manifest file. Either run this command from within a Scarb project \
             or specify it using `--manifest` argument"
        ));
    };
    let env = if let Some(ws) = ws {
        dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
    } else {
        None
    };
    Ok((manifest, env))
}

fn get_world_address(
    args: &Args,
    manifest: &Manifest,
    env_metadata: Option<&Environment>,
) -> anyhow::Result<FieldElement> {
    if let Some(address) = args.world_address {
        return Ok(address);
    }

    if let Some(world_address) = env_metadata.and_then(|env| env.world_address()) {
        return Ok(FieldElement::from_str(world_address)?);
    }

    if let Some(address) = manifest.world.address {
        Ok(address)
    } else {
        Err(anyhow!(
            "Could not find World address. Please specify it with --world, or in manifest.json or \
             [tool.dojo.env] in Scarb.toml"
        ))
    }
}
