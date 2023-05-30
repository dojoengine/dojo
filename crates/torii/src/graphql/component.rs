use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Value, Name};
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
                (Name::new("id"), "ID"),
                (Name::new("name"), "String"),
                (Name::new("address"), "Address"),
                (Name::new("classHash"), "FieldElement"),
                (Name::new("transactionHash"), "FieldElement"),
                (Name::new("storageSchema"), "String"),
                (Name::new("createdAt"), "DateTime"),
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
                        (Name::new("id"), Value::from(component.id)),
                        (Name::new("name"), Value::from(component.name)),
                        (Name::new("address"), Value::from(component.address)),
                        (Name::new("classHash"), Value::from(component.class_hash)),
                        (Name::new("transactionHash"), Value::from(component.transaction_hash)),
                        (Name::new("storageSchema"), Value::from(component.storage_schema)),
                        (
                            Name::new("createdAt"),
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
