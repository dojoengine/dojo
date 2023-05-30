use std::borrow::Cow;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::system_call::system_calls_by_system_id;
use super::utils::value_accessor::ObjectAccessor;
use super::{ObjectTraitInstance, ObjectTraitStatic, TypeMapping, ValueMapping};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
}

pub struct SystemObject {
    pub field_type_mapping: TypeMapping,
}

impl ObjectTraitStatic for SystemObject {
    fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), "ID"),
                (Name::new("name"), "String"),
                (Name::new("address"), "Address"),
                (Name::new("classHash"), "FieldElement"),
                (Name::new("transactionHash"), "FieldElement"),
                (Name::new("createdAt"), "DateTime"),
            ]),
        }
    }

    fn from(field_type_mapping: TypeMapping) -> Self {
        Self { field_type_mapping }
    }
}

impl ObjectTraitInstance for SystemObject {
    fn name(&self) -> &str {
        "system"
    }

    fn type_name(&self) -> &str {
        "System"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.string()?.replace('\"', "");
                    let system_values = system_by_id(&mut conn, &id).await?;
                    Ok(Some(FieldValue::owned_any(system_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCalls", TypeRef::named_nn_list_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let system_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;

                let accessor = ObjectAccessor(Cow::Borrowed(system_values));
                let id = accessor.try_get("id")?;
                let system_calls = system_calls_by_system_id(&mut conn, id.string()?).await?;

                Ok(Some(FieldValue::list(system_calls.into_iter().map(FieldValue::owned_any))))
            })
        })])
    }
}

pub async fn system_by_id(conn: &mut PoolConnection<Sqlite>, id: &str) -> Result<ValueMapping> {
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
    .fetch_one(conn)
    .await?;

    Ok(value_mapping(system))
}

fn value_mapping(system: System) -> ValueMapping {
    IndexMap::from([
        (Name::new("id"), Value::from(system.id)),
        (Name::new("name"), Value::from(system.name)),
        (Name::new("address"), Value::from(system.address)),
        (Name::new("classHash"), Value::from(system.class_hash)),
        (Name::new("transactionHash"), Value::from(system.transaction_hash)),
        (
            Name::new("createdAt"),
            Value::from(system.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        ),
    ])
}
