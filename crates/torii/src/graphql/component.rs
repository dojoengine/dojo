use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::pool::PoolConnection;
use sqlx::{FromRow, Pool, Result, Sqlite};

use super::types::ScalarType;
use super::utils::{format_name, remove_quotes};
use super::{ObjectTrait, TypeMapping, ValueMapping};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub storage_definition: String,
    pub created_at: DateTime<Utc>,
}

pub struct ComponentObject {
    pub field_type_mapping: TypeMapping,
    pub storage_names: Vec<String>,
}

impl ComponentObject {
    // Storage names are passed in on new because
    // it builds the related fields dynamically
    pub fn new(storage_names: Vec<String>) -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (Name::new("id"), TypeRef::ID.to_string()),
                (Name::new("name"), TypeRef::STRING.to_string()),
                (Name::new("address"), ScalarType::ADDRESS.to_string()),
                (Name::new("classHash"), ScalarType::FELT.to_string()),
                (Name::new("transactionHash"), ScalarType::FELT.to_string()),
                (Name::new("storageDefinition"), TypeRef::STRING.to_string()),
                (Name::new("createdAt"), ScalarType::DATE_TIME.to_string()),
            ]),
            storage_names,
        }
    }
}

impl ObjectTrait for ComponentObject {
    fn name(&self) -> &str {
        "component"
    }

    fn type_name(&self) -> &str {
        "Component"
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = remove_quotes(ctx.args.try_get("id")?.string()?);
                    let component_values = component_by_id(&mut conn, &id).await?;
                    Ok(Some(FieldValue::owned_any(component_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID))),
        ]
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(
            self.storage_names
                .iter()
                .map(|storage| {
                    let (name, type_name) = format_name(storage);
                    Field::new(name, TypeRef::named(type_name), |_| {
                        FieldFuture::new(async move {
                            // TODO: implement
                            Ok(Some(Value::Null))
                        })
                    })
                })
                .collect(),
        )
    }
}

async fn component_by_id(conn: &mut PoolConnection<Sqlite>, id: &str) -> Result<ValueMapping> {
    let component: Component =
        sqlx::query_as("SELECT * FROM components WHERE id = $1").bind(id).fetch_one(conn).await?;

    Ok(value_mapping(component))
}

#[allow(dead_code)]
pub async fn components(conn: &mut PoolConnection<Sqlite>) -> Result<Vec<ValueMapping>> {
    let components: Vec<Component> =
        sqlx::query_as("SELECT * FROM components").fetch_all(conn).await?;

    Ok(components.into_iter().map(value_mapping).collect())
}

fn value_mapping(component: Component) -> ValueMapping {
    IndexMap::from([
        (Name::new("id"), Value::from(component.id)),
        (Name::new("name"), Value::from(component.name)),
        (Name::new("address"), Value::from(component.address)),
        (Name::new("classHash"), Value::from(component.class_hash)),
        (Name::new("transactionHash"), Value::from(component.transaction_hash)),
        (Name::new("storageDefinition"), Value::from(component.storage_definition)),
        (
            Name::new("createdAt"),
            Value::from(component.created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        ),
    ])
}
