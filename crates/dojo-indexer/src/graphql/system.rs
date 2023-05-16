use juniper::{graphql_object, FieldResult};
use chrono::{DateTime, Utc};

use super::server::Context;

pub struct System {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

#[graphql_object(context = Context)]
impl System {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn class_hash(&self) -> &str {
        &self.class_hash
    }

    pub fn transaction_hash(&self) -> &str {
        &self.transaction_hash
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub async fn system_calls(
        &self,
        context: &Context,
    ) -> FieldResult<Vec<super::system_call::SystemCall>> {
        super::system_call::system_calls_by_system(context, self.id.clone()).await
    }
}

pub async fn system(context: &Context, id: String) -> FieldResult<System> {
    let mut conn = context.pool.acquire().await.unwrap();

    let system = sqlx::query_as!(
        System,
        r#"
            SELECT 
                id,
                name,
                address,
                class_hash,
                transaction_hash,
                created_at as "created_at: _"
            FROM systems WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await?;

    Ok(system)
}

pub async fn systems(context: &Context) -> FieldResult<Vec<System>> {
    let mut conn = context.pool.acquire().await.unwrap();

    let systems = sqlx::query_as!(
        System,
        r#"
            SELECT 
                id,
                name,
                address,
                class_hash,
                transaction_hash,
                created_at as "created_at: _" 
            FROM systems
        "#
    )
    .fetch_all(&mut conn)
    .await?;

    Ok(systems)
}
