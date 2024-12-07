use std::net::{IpAddr, SocketAddr};

use hyper::{Body, Request, Response, StatusCode};
use tracing::error;

use super::Handler;

pub(crate) const LOG_TARGET: &str = "torii::server::handlers::static";

pub struct StaticHandler {
    client_ip: IpAddr,
    artifacts_addr: Option<SocketAddr>,
}

impl StaticHandler {
    pub fn new(client_ip: IpAddr, artifacts_addr: Option<SocketAddr>) -> Self {
        Self { client_ip, artifacts_addr }
    }
}

#[async_trait::async_trait]
impl Handler for StaticHandler {
    fn should_handle(&self, req: &Request<Body>) -> bool {
        req.uri().path().starts_with("/static")
    }

    async fn handle(&self, req: Request<Body>) -> Response<Body> {
        if let Some(artifacts_addr) = self.artifacts_addr {
            let artifacts_addr = format!("http://{}", artifacts_addr);
            match crate::proxy::GRAPHQL_PROXY_CLIENT
                .call(self.client_ip, &artifacts_addr, req)
                .await
            {
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
