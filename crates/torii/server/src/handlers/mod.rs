pub mod graphql;
pub mod grpc;
pub mod sql;
pub mod static_files;

use hyper::{Body, Request, Response};

#[async_trait::async_trait]
pub trait Handler: Send + Sync {
    // Check if this handler can handle the given request
    fn can_handle(&self, req: &Request<Body>) -> bool;

    // Handle the request
    async fn handle(&self, req: Request<Body>) -> Response<Body>;
}
