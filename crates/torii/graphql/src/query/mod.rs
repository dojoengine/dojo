use std::str::FromStr;

use async_graphql::dynamic::TypeRef;
use async_graphql::{Name, Value};
use chrono::{DateTime, Utc};
use convert_case::{Case, Casing};
use dojo_types::primitive::{Primitive, SqlType};
use regex::Regex;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};
use torii_core::constants::SQL_FELT_DELIMITER;

use crate::constants::{
    BOOLEAN_TRUE, ENTITY_ID_COLUMN, EVENT_MESSAGE_ID_COLUMN, INTERNAL_ENTITY_ID_KEY,
};
use crate::object::model_data::ModelMember;
use crate::types::{TypeData, TypeMapping, ValueMapping};

pub mod data;
pub mod filter;
pub mod order;

pub async fn type_mapping_query(
    conn: &mut SqliteConnection,
    model_id: &str,
) -> sqlx::Result<TypeMapping> {
    let model_members = fetch_model_members(conn, model_id).await?;
    let (root_members, nested_members): (Vec<&ModelMember>, Vec<&ModelMember>) =
        model_members.iter().partition(|member| member.model_idx == 0);

    build_type_mapping(&root_members, &nested_members)
}

async fn fetch_model_members(
    conn: &mut SqliteConnection,
    model_id: &str,
) -> sqlx::Result<Vec<ModelMember>> {
    sqlx::query_as(
        r#"
        SELECT
            id,
            model_id,
            model_idx,
            name,
            type AS ty,
            type_enum,
            key,
            executed_at,
            created_at
        from model_members WHERE model_id = ?
        "#,
    )
    .bind(model_id)
    .fetch_all(conn)
    .await
}

fn build_type_mapping(
    root_members: &[&ModelMember],
    nested_members: &[&ModelMember],
) -> sqlx::Result<TypeMapping> {
    let type_mapping: TypeMapping = root_members
        .iter()
        .map(|&member| {
            let type_data = member_to_type_data(member, nested_members);
            Ok((Name::new(&member.name), type_data))
        })
        .collect::<sqlx::Result<TypeMapping>>()?;

    Ok(type_mapping)
}

fn member_to_type_data(member: &ModelMember, nested_members: &[&ModelMember]) -> TypeData {
    // TODO: convert sql -> Ty directly
    match member.type_enum.as_str() {
        "Primitive" => TypeData::Simple(TypeRef::named(&member.ty)),
        "ByteArray" => TypeData::Simple(TypeRef::named("ByteArray")),
        "Array" => TypeData::List(Box::new(member_to_type_data(
            nested_members
                .iter()
                .find(|&nested_member| {
                    nested_member.model_id == member.model_id
                        && nested_member.id.ends_with(&member.name)
                        // TEMP FIX: refer to parse_nested_type
                        && nested_member
                            .id
                            .split('$')
                            .collect::<Vec<_>>()
                            .starts_with(&member.id.split('$').collect::<Vec<_>>())
                })
                .expect("Array type should have nested type"),
            nested_members,
        ))),
        // Enums that do not have a nested member are considered as a simple Enum
        "Enum"
            if !nested_members.iter().any(|&nested_member| {
                nested_member.model_id == member.model_id
                    && nested_member.id.ends_with(&member.name)
                    && nested_member
                        .id
                        .split('$')
                        .collect::<Vec<_>>()
                        .starts_with(&member.id.split('$').collect::<Vec<_>>())
            }) =>
        {
            TypeData::Simple(TypeRef::named("Enum"))
        }
        _ => parse_nested_type(member, nested_members),
    }
}

fn parse_nested_type(member: &ModelMember, nested_members: &[&ModelMember]) -> TypeData {
    let nested_mapping: TypeMapping = nested_members
        .iter()
        .filter_map(|&nested_member| {
            if member.model_id == nested_member.model_id
            && nested_member.id.ends_with(&member.name)
            // TEMP FIX: a nested member that has the same name as another nested member
                // and that both have parents that start with the same id (Model$Test1 and Model$Test2)
                // will end up being assigned to the wrong parent
                && nested_member
                    .id
                    .split('$')
                    .take(nested_member.id.split('$').count() - 1)
                    .collect::<Vec<_>>()
                    .eq(&member.id.split('$').collect::<Vec<_>>())
            {
                // if the nested member is an Enum and the member is an Enum, we need to inject the
                // Enum type in order to have a "option" field in the nested Enum
                // for the enum variant
                if nested_member.type_enum == "Enum"
                    && nested_member.name == "option"
                    && member.type_enum == "Enum"
                {
                    return Some((Name::new("option"), TypeData::Simple(TypeRef::named("Enum"))));
                }

                let type_data = member_to_type_data(nested_member, nested_members);
                Some((Name::new(&nested_member.name), type_data))
            } else {
                None
            }
        })
        .collect();

    let model_name = member.id.split('$').next().unwrap();
    // sanitizes the member type string
    // for eg. Position_Array<Vec2> -> Position_ArrayVec2
    // Position_(u8, Vec2) -> Position_u8Vec2
    let re = Regex::new(r"[, ()<>-]").unwrap();
    let sanitized_model_name = model_name.replace('-', "_");
    let sanitized_member_type_name = re.replace_all(&member.ty, "");
    let namespaced = format!("{}_{}", sanitized_model_name, sanitized_member_type_name);
    TypeData::Nested((TypeRef::named(namespaced), nested_mapping))
}

fn remove_hex_leading_zeros(value: Value) -> Value {
    if let Value::String(str_val) = &value {
        if !str_val.starts_with("0x") {
            return value;
        }
        let hex_part = str_val.trim_start_matches("0x");
        let trimmed_hex = hex_part.trim_start_matches('0');
        Value::String(format!("0x{:0>1}", trimmed_hex))
    } else {
        value
    }
}

pub fn value_mapping_from_row(
    row: &SqliteRow,
    types: &TypeMapping,
    is_external: bool,
) -> sqlx::Result<ValueMapping> {
    let mut value_mapping = types
        .iter()
        .filter(|(_, type_data)| {
            type_data.is_simple()
            // ignore Enum fields because the column is not stored in this row. we inejct it later
            // && !(type_data.type_ref().to_string() == "Enum")
        })
        .map(|(field_name, type_data)| {
            let mut value =
                fetch_value(row, field_name, &type_data.type_ref().to_string(), is_external)?;

            // handles felt arrays stored as string (ex: keys)
            if let (TypeRef::List(_), Value::String(s)) = (&type_data.type_ref(), &value) {
                let mut felts: Vec<_> = s.split(SQL_FELT_DELIMITER).map(Value::from).collect();
                felts.pop(); // removes empty item
                value = Value::List(felts);
            }

            Ok((Name::new(field_name), value))
        })
        .collect::<sqlx::Result<ValueMapping>>()?;

    // entity_id is not part of a model's type_mapping but needed to relate to parent entity
    if let Ok(entity_id) = row.try_get::<String, &str>(ENTITY_ID_COLUMN) {
        value_mapping.insert(Name::new(INTERNAL_ENTITY_ID_KEY), Value::from(entity_id));
    } else if let Ok(event_message_id) = row.try_get::<String, &str>(EVENT_MESSAGE_ID_COLUMN) {
        value_mapping.insert(Name::new(INTERNAL_ENTITY_ID_KEY), Value::from(event_message_id));
    }

    Ok(value_mapping)
}

fn fetch_value(
    row: &SqliteRow,
    field_name: &str,
    type_name: &str,
    is_external: bool,
) -> sqlx::Result<Value> {
    let column_name = if is_external {
        format!("external_{}", field_name)
    } else {
        field_name.to_string().to_case(Case::Snake)
    };

    match Primitive::from_str(type_name) {
        // fetch boolean
        Ok(Primitive::Bool(_)) => {
            Ok(Value::from(matches!(row.try_get::<i64, &str>(&column_name)?, BOOLEAN_TRUE)))
        }
        // fetch integer/string base on sql type
        Ok(ty) => match ty.to_sql_type() {
            SqlType::Integer => row.try_get::<i64, &str>(&column_name).map(Value::from),
            SqlType::Text => Ok(remove_hex_leading_zeros(
                row.try_get::<String, &str>(&column_name).map(Value::from)?,
            )),
        },
        // fetch everything else
        _ => {
            let value = match type_name {
                "DateTime" => {
                    let dt = row
                        .try_get::<DateTime<Utc>, &str>(&column_name)
                        .expect("Should be a stored as UTC Datetime")
                        .to_rfc3339();
                    Value::from(dt)
                }
                _ => {
                    let s = row.try_get::<String, &str>(&column_name)?;
                    Value::from(s)
                }
            };
            Ok(value)
        }
    }
}
