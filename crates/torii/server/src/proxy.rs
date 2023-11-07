use std::convert::Infallible;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use http::{HeaderName, Method};
use hyper::server::conn::AddrStream;
use hyper::service::make_service_fn;
use hyper::{Body, Request, Response, Server, StatusCode};
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};

const DEFAULT_ALLOW_HEADERS: [&str; 7] = [
    "accept",
    "origin",
    "content-type",
    "access-control-allow-origin",
    "x-grpc-web",
    "x-grpc-timeout",
    "x-user-agent",
];
const DEFAULT_EXPOSED_HEADERS: [&str; 3] =
    ["grpc-status", "grpc-message", "grpc-status-details-bin"];
const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

// async fn model_registered_listener(notify_restart: Arc<Notify>) {
//     while (SimpleBroker::<Model>::subscribe().next().await).is_some() {
//         notify_restart.notify_one();
//     }
// }

fn debug_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let body_str = format!("{:?}", req);
    Ok(Response::new(Body::from(body_str)))
}

async fn handle(
    client_ip: IpAddr,
    grpc_addr: String,
    graphql_addr: String,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    if req.uri().path().starts_with("/grpc") {
        match hyper_reverse_proxy::call(client_ip, &grpc_addr, req).await {
            Ok(response) => Ok(response),
            Err(_error) => Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()),
        }
    } else if req.uri().path().starts_with("/graphql") {
        match hyper_reverse_proxy::call(client_ip, &graphql_addr, req).await {
            Ok(response) => Ok(response),
            Err(_error) => Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()),
        }
    } else {
        debug_request(req)
    }
}

pub async fn new(
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    addr: SocketAddr,
    allowed_origins: Vec<String>,
    grpc_addr: SocketAddr,
    graphql_addr: SocketAddr,
) -> impl Future<Output = Result<(), hyper::Error>> + 'static {
    let make_svc = make_service_fn(move |conn: &AddrStream| {
        let remote_addr = conn.remote_addr().ip();
        let grpc_addr = format!("http://{}", grpc_addr);
        let graphql_addr = format!("http://{}", graphql_addr);

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

        let cors = match allowed_origins.as_slice() {
            [origin] if origin == "*" => cors.allow_origin(AllowOrigin::mirror_request()),
            origins => cors.allow_origin(
                origins.iter().map(|o| o.parse().expect("valid origin")).collect::<Vec<_>>(),
            ),
        };

        let service = ServiceBuilder::new().layer(cors).service_fn(move |req| {
            handle(remote_addr, grpc_addr.clone(), graphql_addr.clone(), req)
        });

        async { Ok::<_, Infallible>(service) }
    });

    let server = Server::bind(&addr).serve(make_svc);
    server.with_graceful_shutdown(async move {
        // Wait for the shutdown signal
        shutdown_rx.recv().await.ok();
    })
}
