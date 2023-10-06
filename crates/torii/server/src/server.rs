use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::Poll;

use either::Either;
use hyper::service::{make_service_fn, Service};
use hyper::Uri;
use sqlx::{Pool, Sqlite};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;
use tokio::sync::mpsc::Receiver as BoundedReceiver;
use torii_grpc::protos;
use warp::Filter;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

// TODO: check if there's a nicer way to implement this
pub async fn spawn_server(
    addr: &SocketAddr,
    pool: &Pool<Sqlite>,
    world_address: FieldElement,
    block_receiver: BoundedReceiver<u64>,
    provider: Arc<JsonRpcClient<HttpTransport>>,
) -> anyhow::Result<()> {
    let world_server =
        torii_grpc::server::DojoWorld::new(pool.clone(), block_receiver, world_address, provider);

    let base_route = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::json(&serde_json::json!({ "success": true })));
    let routes = torii_graphql::route::filter(pool).await.or(base_route);

    let warp = warp::service(routes);
    let tonic = tonic_web::enable(protos::world::world_server::WorldServer::new(world_server));

    hyper::Server::bind(addr)
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
