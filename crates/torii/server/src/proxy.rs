use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use http::header::CONTENT_TYPE;
use http::{HeaderName, Method};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::server::conn::AddrStream;
use hyper::service::make_service_fn;
use hyper::{Body, Client, Request, Response, Server, StatusCode};
use hyper_reverse_proxy::ReverseProxy;
use serde_json::json;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::error;
use sqlx::{SqlitePool, Row, Column, TypeInfo};
use base64::engine::general_purpose::STANDARD;

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
    static ref GRAPHQL_PROXY_CLIENT: ReverseProxy<HttpConnector<GaiResolver>> = {
        ReverseProxy::new(
            Client::builder()
             .build_http(),
        )
    };

    static ref GRPC_PROXY_CLIENT: ReverseProxy<HttpConnector<GaiResolver>> = {
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
    grpc_addr: Option<SocketAddr>,
    artifacts_addr: Option<SocketAddr>,
    graphql_addr: Arc<RwLock<Option<SocketAddr>>>,
    pool: Arc<SqlitePool>,
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
        Self {
            addr,
            allowed_origins,
            grpc_addr,
            graphql_addr: Arc::new(RwLock::new(graphql_addr)),
            artifacts_addr,
            pool,
        }
    }

    pub async fn set_graphql_addr(&self, addr: SocketAddr) {
        let mut graphql_addr = self.graphql_addr.write().await;
        *graphql_addr = Some(addr);
    }

    pub async fn start(
        &self,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), hyper::Error> {
        let addr = self.addr;
        let allowed_origins = self.allowed_origins.clone();
        let grpc_addr = self.grpc_addr;
        let graphql_addr = self.graphql_addr.clone();
        let artifacts_addr = self.artifacts_addr;
        let pool = self.pool.clone();

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

            let pool_clone = pool.clone();
            let graphql_addr_clone = graphql_addr.clone();
            let service = ServiceBuilder::new().option_layer(cors).service_fn(move |req| {
                let pool = pool_clone.clone();
                let graphql_addr = graphql_addr_clone.clone();
                async move {
                    let graphql_addr = graphql_addr.read().await;
                    handle(remote_addr, grpc_addr, artifacts_addr, *graphql_addr, pool, req).await
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
    grpc_addr: Option<SocketAddr>,
    artifacts_addr: Option<SocketAddr>,
    graphql_addr: Option<SocketAddr>,
    pool: Arc<SqlitePool>,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    if req.uri().path().starts_with("/static") {
        if let Some(artifacts_addr) = artifacts_addr {
            let artifacts_addr = format!("http://{}", artifacts_addr);

            return match GRAPHQL_PROXY_CLIENT.call(client_ip, &artifacts_addr, req).await {
                Ok(response) => Ok(response),
                Err(_error) => {
                    error!("{:?}", _error);
                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap())
                }
            };
        } else {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap());
        }
    }

    if req.uri().path().starts_with("/graphql") {
        if let Some(graphql_addr) = graphql_addr {
            let graphql_addr = format!("http://{}", graphql_addr);
            return match GRAPHQL_PROXY_CLIENT.call(client_ip, &graphql_addr, req).await {
                Ok(response) => Ok(response),
                Err(_error) => {
                    error!("{:?}", _error);
                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap())
                }
            };
        } else {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap());
        }
    }

    if let Some(content_type) = req.headers().get(CONTENT_TYPE) {
        if content_type.to_str().unwrap().starts_with("application/grpc") {
            if let Some(grpc_addr) = grpc_addr {
                let grpc_addr = format!("http://{}", grpc_addr);
                return match GRPC_PROXY_CLIENT.call(client_ip, &grpc_addr, req).await {
                    Ok(response) => Ok(response),
                    Err(_error) => {
                        error!("{:?}", _error);
                        Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::empty())
                            .unwrap())
                    }
                };
            } else {
                return Ok(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::empty())
                    .unwrap());
            }
        }
    }

    if req.uri().path().starts_with("/sql") {
        let query = if req.method() == Method::GET {
            // Get the query from URL parameters
            let params = req.uri().query().unwrap_or_default();
            form_urlencoded::parse(params.as_bytes())
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.to_string())
                .unwrap_or_default()
        } else if req.method() == Method::POST {
            // Get the query from request body
            let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap_or_default();
            String::from_utf8(body_bytes.to_vec()).unwrap_or_default()
        } else {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Only GET and POST methods are allowed"))
                .unwrap());
        };

        // Execute the query in a read-only transaction
        return match sqlx::query(&query)
            .fetch_all(&*pool)
            .await {
                Ok(rows) => {
                    let result: Vec<_> = rows.iter()
                        .map(|row| {
                            let mut obj = serde_json::Map::new();
                            for (i, column) in row.columns().iter().enumerate() {
                                let value: serde_json::Value = match column.type_info().name() {
                                    "TEXT" => row.get::<Option<String>, _>(i)
                                        .map_or(serde_json::Value::Null, serde_json::Value::String),
                                    "INTEGER" => row.get::<Option<i64>, _>(i)
                                        .map_or(serde_json::Value::Null, |n| serde_json::Value::Number(n.into())),
                                    "REAL" => row.get::<Option<f64>, _>(i)
                                        .map_or(serde_json::Value::Null, |f| 
                                            serde_json::Number::from_f64(f)
                                                .map_or(serde_json::Value::Null, serde_json::Value::Number)
                                        ),
                                    "BLOB" => row.get::<Option<Vec<u8>>, _>(i)
                                        .map_or(serde_json::Value::Null, |bytes| 
                                            serde_json::Value::String(STANDARD.encode(bytes))
                                        ),
                                    _ => row.get::<Option<String>, _>(i)
                                        .map_or(serde_json::Value::Null, serde_json::Value::String),
                                };
                                obj.insert(column.name().to_string(), value);
                            }
                            serde_json::Value::Object(obj)
                        })
                        .collect();

                    let json = serde_json::to_string(&result).unwrap();
                    
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, "application/json")
                        .body(Body::from(json))
                        .unwrap())
                }
                Err(e) => {
                    error!("SQL query error: {:?}", e);
                    Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("Query error"))
                        .unwrap())
                }
            };
    }

    let json = json!({
        "service": "torii",
        "success": true
    });
    let body = Body::from(json.to_string());
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap();
    Ok(response)
}
