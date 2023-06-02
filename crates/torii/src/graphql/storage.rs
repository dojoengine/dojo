use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::Value;
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

    fn field_resolvers(&self) -> Vec<Field> {
        let name = self.name.clone();
        let type_mapping = self.field_type_mapping.clone();
        vec![
            Field::new(self.name(), TypeRef::named_nn(self.type_name()), move |ctx| {
                let inner_name = name.clone();
                let inner_type_mapping = type_mapping.clone();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = ctx.args.try_get("id")?.i64()?;
                    let storage_values =
                        storage_by_id(&mut conn, &inner_name, &inner_type_mapping, id).await?;
                    Ok(Some(FieldValue::owned_any(storage_values)))
                })
            })
            .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::INT))),
        ]
    }
}

async fn storage_by_id(
    conn: &mut PoolConnection<Sqlite>,
    name: &str,
    fields: &TypeMapping,
    id: i64,
) -> Result<ValueMapping> {
    let query = format!("SELECT * FROM storage_{} WHERE id = ?", name);
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
        value_mapping.insert(field_name.clone(), value);
    }

    Ok(value_mapping)
}
