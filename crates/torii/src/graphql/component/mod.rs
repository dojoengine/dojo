// pub mod instance;

use std::collections::HashMap;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::{FieldTypeMapping, FieldValueMapping, ObjectTraitInstance, ObjectTraitStatic};

#[derive(FromRow, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub name: String,
    pub address: String,
    pub class_hash: String,
    pub transaction_hash: String,
    pub gql_schema: String,
    pub created_at: DateTime<Utc>,
}

pub struct ComponentObject {
    pub field_type_mappings: FieldTypeMapping,
}

impl ObjectTraitStatic for ComponentObject {
    fn new() -> Self {
        Self {
            field_type_mappings: HashMap::from([
                (String::from("id"), String::from("ID")),
                (String::from("name"), String::from("String")),
                (String::from("address"), String::from("Address")),
                (String::from("classHash"), String::from("FieldElement")),
                (String::from("transactionHash"), String::from("FieldElement")),
                (String::from("gqlSchema"), String::from("String")),
                (String::from("createdAt"), String::from("DateTime")),
            ]),
        }
    }

    fn from(field_type_mappings: FieldTypeMapping) -> Self {
        Self { field_type_mappings }
    }
}

impl ObjectTraitInstance for ComponentObject {
    fn name(&self) -> &str {
        "component"
    }

    fn type_name(&self) -> &str {
        "Component"
    }

    fn field_type_mappings(&self) -> &FieldTypeMapping {
        &self.field_type_mappings
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.get("id").expect("id not found");

                    let component: Component =
                        sqlx::query_as("SELECT * FROM components WHERE id = ?")
                            .bind(id.string()?)
                            .fetch_one(&mut conn)
                            .await?;

                    let result: FieldValueMapping = HashMap::from([
                        (String::from("id"), Value::from(component.id)),
                        (String::from("name"), Value::from(component.name)),
                        (String::from("address"), Value::from(component.address)),
                        (String::from("classHash"), Value::from(component.class_hash)),
                        (String::from("transactionHash"), Value::from(component.transaction_hash)),
                        (String::from("gqlSchema"), Value::from(component.gql_schema)),
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
