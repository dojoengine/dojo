use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use prost::Message;
use sqlx::{Pool, Sqlite};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tokio::sync::mpsc::Receiver;
use tonic::client::GrpcService;
use url::Url;
use warp::filters::path::FullPath;
use warp::Filter;

use crate::protos::world::world_server::World;
use crate::protos::{self};

#[derive(Debug)]
struct InvalidProtobufError;

impl warp::reject::Reject for InvalidProtobufError {}

pub fn filter(
    pool: Pool<Sqlite>,
    block_receiver: Receiver<u64>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(
        Url::parse("http://localhost:5050").unwrap(),
    )));

    let world_server = crate::server::DojoWorld::new(pool, provider, block_receiver);
    let server = protos::world::world_server::WorldServer::new(world_server.clone());

    warp::path("grpc").and(warp::post()).and_then(move |body: Bytes| {
        let world = world_server.clone();
        async move {
            let request = match protos::world::MetadataRequest::decode(body) {
                Ok(req) => req,
                Err(_) => return Err(warp::reject::custom(InvalidProtobufError)),
            };

            let response = world.world_metadata(tonic::Request::new(request)).await.unwrap();
            let meta_reply = response.into_inner();

            let mut bytes = BytesMut::new();
            meta_reply.encode(&mut bytes).unwrap();

            Ok::<_, warp::reject::Rejection>(warp::reply::with_header(
                bytes.to_vec(),
                "content-type",
                "application/octet-stream",
            ))
        }
    })
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] http::Error),
}

impl warp::reject::Reject for Error {}

// // `warp` filter for extracting the `http::Request` from the incoming `warp` request.
// pub fn http_request()
// -> impl Filter<Extract = (http::Request<Bytes>,), Error = warp::reject::Rejection> + Copy {
//     // Reference: https://github.com/seanmonstar/warp/issues/139#issuecomment-853153712
//     warp::any()
//         .and(warp::method())
//         .and(warp::filters::path::full())
//         .and(warp::filters::query::raw())
//         .and(warp::header::headers_cloned())
//         .and(warp::body::bytes())
//         .and_then(|method, path: FullPath, query, headers, bytes| async move {
//             let uri = http::uri::Builder::new()
//                 .path_and_query(format!("{}?{}", path.as_str(), query))
//                 .build()
//                 .map_err(Error::from)?;

//             let mut request = http::Request::builder()
//                 .method(method)
//                 .uri(uri)
//                 .body(bytes)
//                 .map_err(Error::from)?;

//             *request.headers_mut() = headers;

//             Ok::<http::Request<Bytes>, warp::reject::Rejection>(request)
//         })
// }
