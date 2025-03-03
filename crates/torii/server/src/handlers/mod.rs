pub mod graphql;
pub mod grpc;
pub mod mcp;
pub mod sql;
pub mod static_files;

use std::net::IpAddr;

use hyper::{Body, Request, Response};

#[async_trait::async_trait]
pub trait Handler: Send + Sync + std::fmt::Debug {
    // Check if this handler should handle the given request
    fn should_handle(&self, req: &Request<Body>) -> bool;

    // Handle the request
    async fn handle(&self, req: Request<Body>, client_addr: IpAddr) -> Response<Body>;
}
