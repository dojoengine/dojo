use juniper::{graphql_object, FieldResult};

use super::server::Context;

pub struct System {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
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

    // pub async fn entity_state_updates(&self, context: &Context) ->
    // FieldResult<Vec<super::entity_state_update::EntityStateUpdate>> {
    //     super::entity_state_update::entity_state_updates_by_system(context,
    // self.id.clone()).await }

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
            SELECT * FROM systems WHERE id = $1
        "#,
        id
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();

    Ok(system)
}

pub async fn systems(context: &Context) -> FieldResult<Vec<System>> {
    let mut conn = context.pool.acquire().await.unwrap();

    let systems = sqlx::query_as!(
        System,
        r#"
            SELECT * FROM systems
        "#
    )
    .fetch_all(&mut conn)
    .await
    .unwrap();

    Ok(systems)
}
