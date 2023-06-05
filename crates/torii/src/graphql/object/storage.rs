use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use dojo_world::manifest::Member;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{Error, Pool, Result, Row, Sqlite};

use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::types::ScalarType;

const BOOLEAN_TRUE: i64 = 1;

pub struct StorageObject {
    pub name: String,
    pub type_name: String,
    pub field_type_mapping: TypeMapping,
}

impl StorageObject {
    pub fn new(name: String, type_name: String, field_type_mapping: TypeMapping) -> Self {
        Self { name, type_name, field_type_mapping }
    }
}

impl ObjectTrait for StorageObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn resolvers(&self) -> Vec<Field> {
        let name = self.name.clone();
        let type_mapping = self.field_type_mapping.clone();
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), move |ctx| {
                let inner_name = name.clone();
                let inner_type_mapping = type_mapping.clone();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.i64()?.to_string();
                    let storage_values = storage_by_column(
                        &mut conn,
                        ColumnName::Id,
                        id.as_str(),
                        &inner_name,
                        &inner_type_mapping,
                    )
                    .await?;
                    Ok(Some(FieldValue::owned_any(storage_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::INT))),
        ]
    }
}

#[allow(dead_code)]
pub enum ColumnName {
    Id,
    ComponentId,
    EntityId,
}

impl ColumnName {
    pub fn as_str(&self) -> &str {
        match self {
            ColumnName::Id => "id",
            ColumnName::ComponentId => "component_id",
            ColumnName::EntityId => "entity_id",
        }
    }
}

pub async fn storage_by_column(
    conn: &mut PoolConnection<Sqlite>,
    column_name: ColumnName,
    id: &str,
    name: &str,
    fields: &TypeMapping,
) -> Result<ValueMapping> {
    let query = format!("SELECT * FROM storage_{} WHERE {} = ?", name, column_name.as_str());
    let storage = sqlx::query(&query).bind(id).fetch_one(conn).await?;
    let result = value_mapping_from_row(&storage, fields)?;
    Ok(result)
}

fn value_mapping_from_row(row: &SqliteRow, fields: &TypeMapping) -> Result<ValueMapping> {
    let mut value_mapping = ValueMapping::new();

    // Cairo's data types are stored as either int or str in sqlite db,
    // int's max size is 64bit so we retrieve all types above u64 as str
    for (field_name, field_type) in fields {
        let value = match field_type.as_str() {
            ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64 => {
                let result = row.try_get::<i64, &str>(field_name.as_str());
                Value::from(result?)
            }
            ScalarType::U128 | ScalarType::U250 | ScalarType::U256 | ScalarType::FELT => {
                let result = row.try_get::<String, &str>(field_name.as_str());
                Value::from(result?)
            }
            TypeRef::BOOLEAN => {
                // sqlite stores booleans as 0 or 1
                let result = row.try_get::<i64, &str>(field_name.as_str());
                Value::from(matches!(result?, BOOLEAN_TRUE))
            }
            _ => return Err(Error::TypeNotFound { type_name: field_type.clone() }),
        };

        value_mapping.insert(Name::new(field_name), value);
    }

    Ok(value_mapping)
}

pub fn type_mapping_from_definition(storage_def: &str) -> Result<TypeMapping> {
    let members: Vec<Member> =
        serde_json::from_str(storage_def).map_err(|e| Error::Decode(e.into()))?;
    let field_type_mapping: TypeMapping =
        members.iter().fold(TypeMapping::new(), |mut mapping, member| {
            // TODO: check if member type exists in scalar types
            mapping.insert(Name::new(&member.name), member.ty.to_string());
            mapping
        });
    Ok(field_type_mapping)
}
