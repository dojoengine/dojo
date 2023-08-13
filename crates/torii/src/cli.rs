use std::env;
use std::str::FromStr;

use anyhow::{anyhow, Error};
use camino::Utf8PathBuf;
use clap::Parser;
use dojo_world::manifest::Manifest;
use graphql::server::start_graphql;
use scarb::core::{Config, ManifestMetadata, Workspace};
use serde::Deserialize;
use sqlx::sqlite::SqlitePoolOptions;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use state::sql::Sql;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing_subscriber::fmt;
use url::Url;

use crate::engine::Processors;
use crate::indexer::Indexer;
use crate::processors::register_component::RegisterComponentProcessor;
use crate::processors::register_system::RegisterSystemProcessor;
use crate::processors::store_set_record::StoreSetRecordProcessor;
use crate::state::State;

mod engine;
mod graphql;
mod indexer;
mod processors;
mod state;
mod types;

#[cfg(test)]
mod tests;

pub(crate) fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Option<DojoMetadata> {
    Some(ws.current_package().ok()?.manifest.metadata.dojo())
}

#[derive(Default, Deserialize, Debug, Clone)]
pub(crate) struct DojoMetadata {
    env: Option<Environment>,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct Environment {
    rpc_url: Option<String>,
    account_address: Option<String>,
    private_key: Option<String>,
    keystore_path: Option<String>,
    keystore_password: Option<String>,
    world_address: Option<String>,
}

impl Environment {
    pub fn world_address(&self) -> Option<&str> {
        self.world_address.as_deref()
    }

    pub fn rpc_url(&self) -> Option<&str> {
        self.rpc_url.as_deref()
    }

    pub fn account_address(&self) -> Option<&str> {
        self.account_address.as_deref()
    }

    pub fn private_key(&self) -> Option<&str> {
        self.private_key.as_deref()
    }

    #[allow(dead_code)]
    pub fn keystore_path(&self) -> Option<&str> {
        self.keystore_path.as_deref()
    }

    pub fn keystore_password(&self) -> Option<&str> {
        self.keystore_password.as_deref()
    }
}

impl DojoMetadata {
    pub fn env(&self) -> Option<&Environment> {
        self.env.as_ref()
    }
}
trait MetadataExt {
    fn dojo(&self) -> DojoMetadata;
}

impl MetadataExt for ManifestMetadata {
    fn dojo(&self) -> DojoMetadata {
        self.tool_metadata
            .as_ref()
            .and_then(|e| e.get("dojo"))
            .cloned()
            .map(|v| v.try_into::<DojoMetadata>().unwrap_or_default())
            .unwrap_or_default()
    }
}

/// Dojo World Indexer
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The world to index
    #[arg(short, long)]
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

fn address(arg: &Args, env_metadata: Option<&Environment>) -> Result<FieldElement, Error> {
    if let Some(world_address) = arg.world_address {
        Ok(world_address)
    } else if let Some(world_address) = env_metadata
        .and_then(|env| env.world_address())
        .or(std::env::var("DOJO_WORLD_ADDRESS").ok().as_deref())
    {
        Ok(FieldElement::from_str(world_address)?)
    } else {
        Err(anyhow!(
            "Could not find World address. Please specify it with --world or in the world config."
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
    sqlx::migrate!().run(&pool).await?;

    let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(&args.rpc).unwrap()));

    let mut manifest_path = scarb::ops::find_manifest_path(args.manifest.as_deref())?;
    let config = Config::builder(manifest_path.clone())
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()?;
    let env_metadata = if config.manifest_path().exists() {
        let ws = scarb::ops::read_workspace(config.manifest_path(), &config)?;

        // TODO: Check the updated scarb way to read profile specific values
        dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
    } else {
        None
    };

    println!("{:?}", env_metadata);

    if manifest_path.ends_with("Scarb.toml") {
        manifest_path.pop();
        manifest_path.push("target/dev/manifest.json");
    }

    let manifest = Manifest::load_from_path(manifest_path).expect("Failed to load manifest");
    let world_address_result = address(&args, env_metadata.as_ref());

    let world_address = match world_address_result {
        Ok(world_address) => world_address,
        Err(e) => return Err(e),
    };

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
