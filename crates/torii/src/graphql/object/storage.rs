use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use async_graphql::{Name, Value};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{Error, Pool, QueryBuilder, Result, Row, Sqlite};

use super::component::ComponentMembers;
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
        vec![Field::new(self.name(), TypeRef::named_nn(self.type_name()), move |ctx| {
            let inner_name = name.clone();
            let inner_type_mapping = type_mapping.clone();

            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let storage_values =
                    storage_by_name(&mut conn, &inner_name, &inner_type_mapping).await?;
                Ok(Some(FieldValue::owned_any(storage_values)))
            })
        })]
    }
}

pub async fn storage_by_name(
    conn: &mut PoolConnection<Sqlite>,
    name: &str,
    fields: &TypeMapping,
) -> Result<ValueMapping> {
    let query = format!("SELECT * FROM {}", name);
    let storage = sqlx::query(&query).fetch_one(conn).await?;
    let result = value_mapping_from_row(&storage, fields)?;
    Ok(result)
}

fn value_mapping_from_row(row: &SqliteRow, fields: &TypeMapping) -> Result<ValueMapping> {
    let mut value_mapping = ValueMapping::new();

    // Cairo's data types are stored as either int or str in sqlite db,
    // int's max size is 64bit so we retrieve all types above u64 as str
    for (field_name, field_type) in fields {
        // Column names are prefixed to avoid conflicts with sqlite keywords
        let column_name = format!("external_{}", field_name);

        // Treating everything as text for now, possilbe to have u8 - u64 as int
        let value = match field_type.as_str() {
            ScalarType::U8
            | ScalarType::U16
            | ScalarType::U32
            | ScalarType::U64
            | ScalarType::U128
            | ScalarType::U250
            | ScalarType::U256
            | ScalarType::FELT => {
                let result = row.try_get::<String, &str>(&column_name);
                Value::from(result?)
            }
            TypeRef::BOOLEAN => {
                // sqlite stores booleans as 0 or 1
                let result = row.try_get::<i64, &str>(&column_name);
                Value::from(matches!(result?, BOOLEAN_TRUE))
            }
            _ => return Err(Error::TypeNotFound { type_name: field_type.clone() }),
        };
        value_mapping.insert(Name::new(field_name), value);
    }

    Ok(value_mapping)
}

pub async fn type_mapping_from(
    conn: &mut PoolConnection<Sqlite>,
    component_id: &str,
) -> Result<TypeMapping> {
    let component_members: Vec<ComponentMembers> = sqlx::query_as(
        r#"
                SELECT 
                    component_id,
                    name,
                    type AS ty,
                    slot,
                    offset,
                    created_at
                FROM component_members WHERE component_id = ?
            "#,
    )
    .bind(component_id)
    .fetch_all(conn)
    .await?;

    // TODO: check if type exists in scalar types
    let field_type_mapping =
        component_members.iter().fold(TypeMapping::new(), |mut acc, member| {
            acc.insert(Name::new(member.name.clone()), member.ty.clone());
            acc
        });

    Ok(field_type_mapping)
}
