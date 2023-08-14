use sqlx::{Pool, Sqlite};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use world::world_server::{World, WorldServer};
use world::{MetaReply, MetaRequest};

pub mod world {
    tonic::include_proto!("world");
}

#[derive(Debug)]
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

pub async fn start(pool: Pool<Sqlite>) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let world = DojoWorld::new(pool);

    Server::builder().add_service(WorldServer::new(world)).serve(addr).await?;

    Ok(())
}
