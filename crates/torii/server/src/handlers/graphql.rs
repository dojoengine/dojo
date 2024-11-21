use std::net::{IpAddr, SocketAddr};

use http::StatusCode;
use hyper::{Body, Request, Response};
use tracing::error;

use super::Handler;

pub(crate) const LOG_TARGET: &str = "torii::server::handlers::graphql";

pub struct GraphQLHandler {
    client_ip: IpAddr,
    graphql_addr: Option<SocketAddr>,
}

impl GraphQLHandler {
    pub fn new(client_ip: IpAddr, graphql_addr: Option<SocketAddr>) -> Self {
        Self { client_ip, graphql_addr }
    }
}

#[async_trait::async_trait]
impl Handler for GraphQLHandler {
    fn can_handle(&self, req: &Request<Body>) -> bool {
        req.uri().path().starts_with("/graphql")
    }

    async fn handle(&self, req: Request<Body>) -> Response<Body> {
        if let Some(addr) = self.graphql_addr {
            let graphql_addr = format!("http://{}", addr);
            match crate::proxy::GRAPHQL_PROXY_CLIENT.call(self.client_ip, &graphql_addr, req).await
            {
                Ok(response) => response,
                Err(_error) => {
                    error!(target: LOG_TARGET, "GraphQL proxy error: {:?}", _error);
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
