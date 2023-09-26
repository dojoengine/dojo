use std::convert::Infallible;
use std::env;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::Poll;

use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use clap::Parser;
use dojo_world::manifest::Manifest;
use dojo_world::metadata::{dojo_metadata_from_workspace, Environment};
use either::Either;
use http::Uri;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn, Service};
use hyper::Body;
use scarb::core::Config;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;
use torii_client::contract::world::WorldContractReader;
use torii_core::engine::{Engine, EngineConfig, Processors};
use torii_core::processors::register_model::RegisterModelProcessor;
use torii_core::processors::register_system::RegisterSystemProcessor;
use torii_core::processors::store_set_record::StoreSetRecordProcessor;
use torii_core::sql::Sql;
use tracing::error;
use tracing_subscriber::fmt;
use url::Url;

mod server;

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
    /// Port number for GraphQL/gRPC endpoints
    #[arg(long, default_value = "8080")]
    port: u16,
}

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

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
            Box::new(RegisterModelProcessor),
            Box::new(RegisterSystemProcessor),
            Box::new(StoreSetRecordProcessor),
        ],
        // transaction: vec![Box::new(StoreSystemCallProcessor)],
        ..Processors::default()
    };

    let (block_sender, block_receiver) = tokio::sync::mpsc::channel::<u64>(10);

    let engine = Engine::new(
        &world,
        &db,
        &provider,
        processors,
        EngineConfig { start_block: args.start_block, ..Default::default() },
    );

    let addr = format!("{}:{}", args.host, args.port)
        .parse::<SocketAddr>()
        .expect("able to parse address");

    tokio::select! {
        res = engine.start(cts, block_sender) => {
            if let Err(e) = res {
                error!("Indexer failed with error: {e}");
            }
        }

        res = server::spawn_server(&addr, &pool) => {
            if let Err(e) = res {
                error!("Server failed with error: {e}");
            }
        }

        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down");
        }
    }

    Ok(())
}

// TODO: check if there's a nicer way to implement this
async fn spawn_server(
    addr: &SocketAddr,
    pool: Pool<Sqlite>,
    block_receiver: Receiver<u64>,
) -> anyhow::Result<()> {
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(
        Url::parse("http://localhost:5050").unwrap(),
    )));

    let world_server = torii_grpc::server::DojoWorld::new(pool.clone(), provider, block_receiver);
    let mut tonic = tonic_web::enable(torii_grpc::protos::world::world_server::WorldServer::new(
        world_server.clone(),
    ));

    let base_route = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({ "success": true })));
    let routes = torii_graphql::route::filter(pool).await.or(base_route);

    let warp = warp::service(routes);

    hyper::Server::bind(&addr)
        .serve(make_service_fn(move |_| {
            let mut tonic = tonic.clone();
            let mut warp = warp.clone();

            std::future::ready(Ok::<_, Infallible>(tower::service_fn(
                move |mut req: hyper::Request<hyper::Body>| {
                    let mut uri = req.uri().path().split("/").skip(1);
                    match uri.next() {
                        Some("grpc") => {
                            let grpc_method = uri.collect::<Vec<&str>>().join("/");
                            *req.uri_mut() =
                                Uri::from_str(&format!("/{grpc_method}")).expect("valid uri");

                            Either::Right({
                                let res = tonic.call(req);
                                Box::pin(async move {
                                    let res = res.await.map(|res| res.map(EitherBody::Right))?;
                                    Ok::<_, Error>(res)
                                })
                            })
                        }

                        _ => Either::Left({
                            let res = warp.call(req);
                            Box::pin(async move {
                                let res = res.await.map(|res| res.map(EitherBody::Left))?;
                                Ok::<_, Error>(res)
                            })
                        }),
                    }
                },
            )))
        }))
        .await?;

    Ok(())
}

enum EitherBody<A, B> {
    Left(A),
    Right(B),
}

impl<A, B> http_body::Body for EitherBody<A, B>
where
    A: http_body::Body + Send + Unpin,
    B: http_body::Body<Data = A::Data> + Send + Unpin,
    A::Error: Into<Error>,
    B::Error: Into<Error>,
{
    type Data = A::Data;
    type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

    fn is_end_stream(&self) -> bool {
        match self {
            EitherBody::Left(b) => b.is_end_stream(),
            EitherBody::Right(b) => b.is_end_stream(),
        }
    }

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.get_mut() {
            EitherBody::Left(b) => Pin::new(b).poll_data(cx).map(map_option_err),
            EitherBody::Right(b) => Pin::new(b).poll_data(cx).map(map_option_err),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<http::HeaderMap>, Self::Error>> {
        match self.get_mut() {
            EitherBody::Left(b) => Pin::new(b).poll_trailers(cx).map_err(Into::into),
            EitherBody::Right(b) => Pin::new(b).poll_trailers(cx).map_err(Into::into),
        }
    }
}

fn map_option_err<T, U: Into<Error>>(err: Option<Result<T, U>>) -> Option<Result<T, Error>> {
    err.map(|e| e.map_err(Into::into))
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
