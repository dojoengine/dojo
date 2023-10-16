use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::Poll;

use either::Either;
use http::header::{ACCEPT, ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_TYPE, ORIGIN};
use http::Method;
use hyper::service::{make_service_fn, Service};
use hyper::Uri;
use sqlx::{Pool, Sqlite};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Notify;
use tokio_stream::StreamExt;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Model;
use torii_grpc::protos;
use torii_grpc::server::DojoWorld;
use tracing::info;
use url::Url;
use warp::filters::cors::Builder;
use warp::Filter;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

pub struct Server {
    addr: SocketAddr,
    pool: Pool<Sqlite>,
    world: DojoWorld,
    allowed_origins: Vec<String>,
    external_url: Option<Url>,
}

impl Server {
    pub fn new(
        addr: SocketAddr,
        pool: Pool<Sqlite>,
        block_rx: Receiver<u64>,
        world_address: FieldElement,
        provider: Arc<JsonRpcClient<HttpTransport>>,
        allowed_origins: Vec<String>,
        external_url: Option<Url>,
    ) -> Self {
        let world =
            torii_grpc::server::DojoWorld::new(pool.clone(), block_rx, world_address, provider);

        Self { addr, pool, world, allowed_origins, external_url }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let notify_restart = Arc::new(Notify::new());

        info!("🚀 Torii listening at {}", format!("http://{}", self.addr));
        info!("Graphql playground: {}\n", format!("http://{}/graphql", self.addr));

        tokio::spawn(model_registered_listener(notify_restart.clone()));

        loop {
            let server_handle = tokio::spawn(spawn(
                self.addr,
                self.pool.clone(),
                self.world.clone(),
                notify_restart.clone(),
                self.allowed_origins.clone(),
                self.external_url.clone(),
            ));

            match server_handle.await {
                Ok(Ok(_)) => {
                    // server graceful shutdown, restart
                    continue;
                }
                Ok(Err(e)) => {
                    return Err(e);
                }
                Err(e) => return Err(e.into()),
            };
        }
    }
}

async fn model_registered_listener(notify_restart: Arc<Notify>) {
    while (SimpleBroker::<Model>::subscribe().next().await).is_some() {
        notify_restart.notify_one();
    }
}

// TODO: check if there's a nicer way to implement this
async fn spawn(
    addr: SocketAddr,
    pool: Pool<Sqlite>,
    dojo_world: DojoWorld,
    notify_restart: Arc<Notify>,
    allowed_origins: Vec<String>,
    external_url: Option<Url>,
) -> anyhow::Result<()> {
    let base_route = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({ "success": true })));
    let routes = torii_graphql::route::filter(&pool, external_url)
        .await
        .or(base_route)
        .with(configure_cors(&allowed_origins));

    let warp = warp::service(routes);

    // TODO: apply allowed_origins to tonic grpc
    let tonic =
        tonic_web::enable(protos::world::world_server::WorldServer::new(dojo_world.clone()));

    hyper::Server::bind(&addr)
        .serve(make_service_fn(move |_| {
            let mut tonic = tonic.clone();
            let mut warp = warp.clone();

            std::future::ready(Ok::<_, Infallible>(tower::service_fn(
                move |mut req: hyper::Request<hyper::Body>| {
                    let mut path_iter = req.uri().path().split('/').skip(1);

                    // check the base path
                    match path_iter.next() {
                        // There's a bug in tonic client where the URI path is not respected in
                        // `Endpoint`, but this issue doesn't exist if `torii-client` is compiled to
                        // `wasm32-unknown-unknown`. See: https://github.com/hyperium/tonic/issues/1314
                        Some("grpc") => {
                            let grpc_method = path_iter.collect::<Vec<_>>().join("/");
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
        .with_graceful_shutdown(async {
            notify_restart.notified().await;
        })
        .await?;

    Ok(())
}

fn configure_cors(origins: &Vec<String>) -> Builder {
    if origins.len() == 1 && origins[0] == "*" {
        warp::cors().allow_any_origin()
    } else {
        let origins_str: Vec<&str> = origins.iter().map(|origin| origin.as_str()).collect();
        warp::cors().allow_origins(origins_str)
    }
    .allow_headers(vec![ACCEPT, ORIGIN, CONTENT_TYPE, ACCESS_CONTROL_ALLOW_ORIGIN])
    .allow_methods(&[Method::POST, Method::GET, Method::OPTIONS])
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
