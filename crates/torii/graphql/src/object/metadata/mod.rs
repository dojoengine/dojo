use async_graphql::connection::PageInfo;
use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::{Name, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};

use super::connection::page_info::PageInfoObject;
use super::connection::{connection_arguments, cursor, parse_connection_arguments};
use super::ObjectTrait;
use crate::constants::{
    ID_COLUMN, JSON_COLUMN, METADATA_NAMES, METADATA_TABLE, METADATA_TYPE_NAME,
};
use crate::mapping::METADATA_TYPE_MAPPING;
use crate::query::data::{count_rows, fetch_multiple_rows};
use crate::query::value_mapping_from_row;
use crate::types::{TypeMapping, ValueMapping};

pub mod content;
pub mod social;

pub struct MetadataObject;

impl ObjectTrait for MetadataObject {
    fn name(&self) -> (&str, &str) {
        METADATA_NAMES
    }

    fn type_name(&self) -> &str {
        METADATA_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &METADATA_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(METADATA_TABLE)
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        let type_mapping = self.type_mapping().clone();
        let table_name = self.table_name().unwrap().to_string();

        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            move |ctx| {
                let type_mapping = type_mapping.clone();
                let table_name = table_name.to_string();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let total_count = count_rows(&mut conn, &table_name, &None, &None).await?;
                    let (data, page_info) = fetch_multiple_rows(
                        &mut conn,
                        &table_name,
                        ID_COLUMN,
                        &None,
                        &None,
                        &None,
                        &connection,
                        total_count,
                    )
                    .await?;

                    // convert json field to value_mapping expected by content object
                    let results =
                        metadata_connection_output(&data, &type_mapping, total_count, page_info)?;

                    Ok(Some(Value::Object(results)))
                })
            },
        );

        field = connection_arguments(field);

        Some(field)
    }
}

// NOTE: need to generalize `connection_output` or maybe preprocess to support both predefined
// objects AND dynamic model objects
fn metadata_connection_output(
    data: &[SqliteRow],
    types: &TypeMapping,
    total_count: i64,
    page_info: PageInfo,
) -> sqlx::Result<ValueMapping> {
    let edges = data
        .iter()
        .map(|row| {
            let order = row.try_get::<String, &str>(ID_COLUMN)?;
            let cursor = cursor::encode(&order, &order);
            let mut value_mapping = value_mapping_from_row(row, types, false)?;

            let json_str = row.try_get::<String, &str>(JSON_COLUMN)?;
            let serde_value = serde_json::from_str(&json_str).unwrap_or_default();

            let content = ValueMapping::from([
                extract_str_mapping("name", &serde_value),
                extract_str_mapping("description", &serde_value),
                extract_str_mapping("website", &serde_value),
                extract_str_mapping("icon_uri", &serde_value),
                extract_str_mapping("cover_uri", &serde_value),
                extract_socials_mapping("socials", &serde_value),
            ]);

            value_mapping.insert(Name::new("content"), Value::Object(content));

            let edge = ValueMapping::from([
                (Name::new("node"), Value::Object(value_mapping)),
                (Name::new("cursor"), Value::String(cursor)),
            ]);

            Ok(Value::Object(edge))
        })
        .collect::<sqlx::Result<Vec<Value>>>();

    Ok(ValueMapping::from([
        (Name::new("total_count"), Value::from(total_count)),
        (Name::new("edges"), Value::List(edges?)),
        (Name::new("page_info"), PageInfoObject::value(page_info)),
    ]))
}

fn extract_str_mapping(name: &str, serde_value: &serde_json::Value) -> (Name, Value) {
    if let Some(serde_json::Value::String(str)) = serde_value.get(name) {
        return (Name::new(name), Value::String(str.to_owned()));
    }

    (Name::new(name), Value::Null)
}

fn extract_socials_mapping(name: &str, serde_value: &serde_json::Value) -> (Name, Value) {
    if let Some(serde_json::Value::Object(obj)) = serde_value.get(name) {
        let list = obj
            .iter()
            .map(|(social_name, social_url)| {
                Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(social_name.to_string())),
                    (Name::new("url"), Value::String(social_url.as_str().unwrap().to_string())),
                ]))
            })
            .collect::<Vec<Value>>();

        return (Name::new(name), Value::List(list));
    }

    (Name::new(name), Value::List(vec![]))
}
