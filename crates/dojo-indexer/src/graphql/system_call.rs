use juniper::{graphql_object, FieldResult};
use chrono::{DateTime, Utc};

use super::server::Context;
use super::system;

pub struct SystemCall {
    pub id: i64,
    pub system_id: String,
    pub transaction_hash: String,
    pub data: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[graphql_object(context = Context)]
impl SystemCall {
    pub fn id(&self) -> i32 {
        i32::try_from(self.id).unwrap()
    }

    pub fn system_id(&self) -> &str {
        &self.system_id
    }

    pub fn transaction_hash(&self) -> &str {
        &self.transaction_hash
    }

    pub fn data(&self) -> &Option<String> {
        &self.data
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    async fn system(&self, context: &Context) -> FieldResult<system::System> {
        system::system(context, self.system_id.clone()).await
    }
}

pub async fn system_call(context: &Context, id: i64) -> FieldResult<SystemCall> {
    let mut conn = context.pool.acquire().await.unwrap();

    let system_call = sqlx::query_as!(
        SystemCall,
        r#"
            SELECT 
                id,
                data,
                transaction_hash,
                system_id,
                created_at as "created_at: _"
            FROM system_calls WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(system_call)
}

pub async fn system_calls_by_system(
    context: &Context,
    system_id: String,
) -> FieldResult<Vec<SystemCall>> {
    let mut conn = context.pool.acquire().await.unwrap();

    let system_calls = sqlx::query_as!(
        SystemCall,
        r#"
            SELECT 
                id,
                data,
                transaction_hash,
                system_id,
                created_at as "created_at: _"
            FROM system_calls WHERE system_id = $1
        "#,
        system_id
    )
    .fetch_all(&mut conn)
    .await?;

    Ok(system_calls)
}
