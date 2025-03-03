use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use http::header::CONTENT_TYPE;
use http::{HeaderName, Method};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::server::conn::AddrStream;
use hyper::service::make_service_fn;
use hyper::{Body, Client, Request, Response, Server, StatusCode};
use hyper_reverse_proxy::ReverseProxy;
use serde_json::json;
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::handlers::graphql::GraphQLHandler;
use crate::handlers::grpc::GrpcHandler;
use crate::handlers::mcp::McpHandler;
use crate::handlers::sql::SqlHandler;
use crate::handlers::static_files::StaticHandler;
use crate::handlers::Handler;

const DEFAULT_ALLOW_HEADERS: [&str; 13] = [
    "accept",
    "origin",
    "content-type",
    "access-control-allow-origin",
    "upgrade",
    "x-grpc-web",
    "x-grpc-timeout",
    "x-user-agent",
    "connection",
    "sec-websocket-key",
    "sec-websocket-version",
    "grpc-accept-encoding",
    "grpc-encoding",
];
const DEFAULT_EXPOSED_HEADERS: [&str; 4] =
    ["grpc-status", "grpc-message", "grpc-status-details-bin", "grpc-encoding"];
const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

lazy_static::lazy_static! {
    pub(crate) static ref GRAPHQL_PROXY_CLIENT: ReverseProxy<HttpConnector<GaiResolver>> = {
        ReverseProxy::new(
            Client::builder()
             .build_http(),
        )
    };

    pub(crate) static ref GRPC_PROXY_CLIENT: ReverseProxy<HttpConnector<GaiResolver>> = {
        ReverseProxy::new(
            Client::builder()
             .http2_only(true)
             .build_http(),
        )
    };
}

#[derive(Debug)]
pub struct Proxy {
    addr: SocketAddr,
    allowed_origins: Option<Vec<String>>,
    handlers: Arc<RwLock<Vec<Box<dyn Handler>>>>,
}

impl Proxy {
    pub fn new(
        addr: SocketAddr,
        allowed_origins: Option<Vec<String>>,
        grpc_addr: Option<SocketAddr>,
        graphql_addr: Option<SocketAddr>,
        artifacts_addr: Option<SocketAddr>,
        pool: Arc<SqlitePool>,
    ) -> Self {
        let handlers: Arc<RwLock<Vec<Box<dyn Handler>>>> = Arc::new(RwLock::new(vec![
            Box::new(GraphQLHandler::new(graphql_addr)),
            Box::new(GrpcHandler::new(grpc_addr)),
            Box::new(McpHandler::new(pool.clone())),
            Box::new(SqlHandler::new(pool.clone())),
            Box::new(StaticHandler::new(artifacts_addr)),
        ]));

        Self { addr, allowed_origins, handlers }
    }

    pub async fn set_graphql_addr(&self, addr: SocketAddr) {
        let mut handlers = self.handlers.write().await;
        handlers[0] = Box::new(GraphQLHandler::new(Some(addr)));
    }

    pub async fn start(
        &self,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), hyper::Error> {
        let addr = self.addr;
        let allowed_origins = self.allowed_origins.clone();

        let make_svc = make_service_fn(move |conn: &AddrStream| {
            let remote_addr = conn.remote_addr().ip();

            let cors = CorsLayer::new()
                .max_age(DEFAULT_MAX_AGE)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(
                    DEFAULT_ALLOW_HEADERS
                        .iter()
                        .cloned()
                        .map(HeaderName::from_static)
                        .collect::<Vec<HeaderName>>(),
                )
                .expose_headers(
                    DEFAULT_EXPOSED_HEADERS
                        .iter()
                        .cloned()
                        .map(HeaderName::from_static)
                        .collect::<Vec<HeaderName>>(),
                );

            let cors =
                allowed_origins.clone().map(|allowed_origins| match allowed_origins.as_slice() {
                    [origin] if origin == "*" => cors.allow_origin(AllowOrigin::mirror_request()),
                    origins => cors.allow_origin(
                        origins
                            .iter()
                            .map(|o| {
                                let _ = o.parse::<http::Uri>().expect("Invalid URI");

                                o.parse().expect("Invalid origin")
                            })
                            .collect::<Vec<_>>(),
                    ),
                });

            let handlers = self.handlers.clone();
            let service = ServiceBuilder::new().option_layer(cors).service_fn(move |req| {
                let handlers = handlers.clone();
                async move {
                    let handlers = handlers.read().await;
                    handle(remote_addr, req, &handlers).await
                }
            });

            async { Ok::<_, Infallible>(service) }
        });

        let server = Server::bind(&addr).serve(make_svc);
        server
            .with_graceful_shutdown(async move {
                // Wait for the shutdown signal
                shutdown_rx.recv().await.ok();
            })
            .await
    }
}

async fn handle(
    client_ip: IpAddr,
    req: Request<Body>,
    handlers: &[Box<dyn Handler>],
) -> Result<Response<Body>, Infallible> {
    for handler in handlers.iter() {
        if handler.should_handle(&req) {
            return Ok(handler.handle(req, client_ip).await);
        }
    }

    // Default response if no handler matches
    let json = json!({
        "service": "torii",
        "success": true
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(json.to_string()))
        .unwrap())
}
