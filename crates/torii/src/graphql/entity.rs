use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::{ObjectTraitInstance, ObjectTraitStatic, TypeMapping, ValueMapping};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub partition_id: String,
    pub keys: Option<String>,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

pub struct EntityObject {
    pub field_type_mapping: TypeMapping,
}

impl ObjectTraitStatic for EntityObject {
    fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (String::from("id"), String::from("ID")),
                (String::from("name"), String::from("String")),
                (String::from("partitionId"), String::from("FieldElement")),
                (String::from("keys"), String::from("String")),
                (String::from("transactionHash"), String::from("FieldElement")),
                (String::from("createdAt"), String::from("DateTime")),
            ]),
        }
    }
    fn from(field_type_mapping: TypeMapping) -> Self {
        Self { field_type_mapping }
    }
}

impl ObjectTraitInstance for EntityObject {
    fn name(&self) -> &str {
        "entity"
    }

    fn type_name(&self) -> &str {
        "Entity"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?;

                    let entity: Entity = sqlx::query_as("SELECT * FROM entities WHERE id = ?")
                        .bind(id.string()?)
                        .fetch_one(&mut conn)
                        .await?;

                    let result: ValueMapping = IndexMap::from([
                        (String::from("id"), Value::from(entity.id)),
                        (String::from("name"), Value::from(entity.name)),
                        (String::from("partitionId"), Value::from(entity.partition_id)),
                        (String::from("keys"), Value::from(entity.keys.unwrap_or_default())),
                        (String::from("transactionHash"), Value::from(entity.transaction_hash)),
                        (
                            String::from("createdAt"),
                            Value::from(
                                entity
                                    .created_at
                                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                            ),
                        ),
                    ]);

                    Ok(Some(FieldValue::owned_any(result)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }
}
