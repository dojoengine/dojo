use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::{ObjectTraitInstance, ObjectTraitStatic, TypeMapping, ValueMapping};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub storage_schema: String,
    pub created_at: DateTime<Utc>,
}

pub struct ComponentObject {
    pub field_type_mapping: TypeMapping,
}

impl ObjectTraitStatic for ComponentObject {
    fn new() -> Self {
        Self {
            field_type_mapping: IndexMap::from([
                (String::from("id"), String::from("ID")),
                (String::from("name"), String::from("String")),
                (String::from("address"), String::from("Address")),
                (String::from("classHash"), String::from("FieldElement")),
                (String::from("transactionHash"), String::from("FieldElement")),
                (String::from("storageSchema"), String::from("String")),
                (String::from("createdAt"), String::from("DateTime")),
            ]),
        }
    }

    fn from(field_type_mapping: TypeMapping) -> Self {
        Self { field_type_mapping }
    }
}

impl ObjectTraitInstance for ComponentObject {
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
                    let id = ctx.args.try_get("id")?;

                    let component: Component =
                        sqlx::query_as("SELECT * FROM components WHERE id = ?")
                            .bind(id.string()?)
                            .fetch_one(&mut conn)
                            .await?;

                    let result: ValueMapping = IndexMap::from([
                        (String::from("id"), Value::from(component.id)),
                        (String::from("name"), Value::from(component.name)),
                        (String::from("address"), Value::from(component.address)),
                        (String::from("classHash"), Value::from(component.class_hash)),
                        (String::from("transactionHash"), Value::from(component.transaction_hash)),
                        (String::from("storageSchema"), Value::from(component.storage_schema)),
                        (
                            String::from("createdAt"),
                            Value::from(
                                component
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
