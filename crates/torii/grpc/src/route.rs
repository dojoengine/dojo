use bytes::{Bytes, BytesMut};
use prost::Message;
use sqlx::{Pool, Sqlite};
use tonic::{Request, Response, Status};
use warp::Filter;
use world::world_server::World;
use world::{MetaReply, MetaRequest};

pub mod world {
    tonic::include_proto!("world");
}

#[derive(Clone, Debug)]
pub struct DojoWorld {
    pool: Pool<Sqlite>,
}

impl DojoWorld {
    fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[tonic::async_trait]
impl World for DojoWorld {
    async fn meta(
        &self,
        request: Request<MetaRequest>, // Accept request of type MetaRequest
    ) -> Result<Response<MetaReply>, Status> {
        let id = request.into_inner().id;

        let (world_address, world_class_hash, executor_address, executor_class_hash): (
            String,
            String,
            String,
            String,
        ) = sqlx::query_as(
            "SELECT world_address, world_class_hash, executor_address, executor_class_hash FROM \
             worlds WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => Status::not_found("World not found"),
            _ => Status::internal("Internal error"),
        })?;

        let reply = world::MetaReply {
            world_address,
            world_class_hash,
            executor_address,
            executor_class_hash,
        };

        Ok(Response::new(reply)) // Send back our formatted greeting
    }
}

#[derive(Debug)]
struct InvalidProtobufError;

impl warp::reject::Reject for InvalidProtobufError {}

pub fn filter(
    pool: &Pool<Sqlite>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let world = DojoWorld::new(pool.clone());

    warp::path("grpc").and(warp::post()).and(warp::body::bytes()).and_then(move |body: Bytes| {
        let world = world.clone();
        async move {
            let request = match MetaRequest::decode(body) {
                Ok(req) => req,
                Err(_) => return Err(warp::reject::custom(InvalidProtobufError)),
            };
            let response = world.meta(tonic::Request::new(request)).await.unwrap();
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
