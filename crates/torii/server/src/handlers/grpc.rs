use std::net::{IpAddr, SocketAddr};

use http::header::CONTENT_TYPE;
use hyper::{Body, Request, Response, StatusCode};
use tracing::error;

use super::Handler;

pub(crate) const LOG_TARGET: &str = "torii::server::handlers::grpc";

#[derive(Debug)]
pub struct GrpcHandler {
    client_ip: IpAddr,
    grpc_addr: Option<SocketAddr>,
}

impl GrpcHandler {
    pub fn new(client_ip: IpAddr, grpc_addr: Option<SocketAddr>) -> Self {
        Self { client_ip, grpc_addr }
    }
}

#[async_trait::async_trait]
impl Handler for GrpcHandler {
    fn should_handle(&self, req: &Request<Body>) -> bool {
        req.headers()
            .get(CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .map(|ct| ct.starts_with("application/grpc"))
            .unwrap_or(false)
    }

    async fn handle(&self, req: Request<Body>) -> Response<Body> {
        if let Some(grpc_addr) = self.grpc_addr {
            let grpc_addr = format!("http://{}", grpc_addr);
            match crate::proxy::GRPC_PROXY_CLIENT.call(self.client_ip, &grpc_addr, req).await {
                Ok(response) => response,
                Err(_error) => {
                    error!(target: LOG_TARGET, "{:?}", _error);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                }
            }
        } else {
            Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap()
        }
    }
}
