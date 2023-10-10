use std::str::FromStr;

use async_graphql::dynamic::TypeRef;
use async_graphql::{Name, Value};
use constants::BOOLEAN_TRUE;
use dojo_types::primitive::Primitive;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite};

use crate::object::model_data::ModelMember;
use crate::types::{TypeData, TypeMapping, ValueMapping};

pub mod constants;
pub mod data;
pub mod filter;
pub mod order;

pub async fn type_mapping_query(
    conn: &mut PoolConnection<Sqlite>,
    model_id: &str,
) -> sqlx::Result<TypeMapping> {
    let model_members = fetch_model_members(conn, model_id).await?;

    let (root_members, nested_members): (Vec<&ModelMember>, Vec<&ModelMember>) =
        model_members.iter().partition(|member| member.model_idx == 0);

    build_type_mapping(&root_members, &nested_members)
}

async fn fetch_model_members(
    conn: &mut PoolConnection<Sqlite>,
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
        "Enum" => TypeData::Simple(TypeRef::named("Enum")),
        _ => parse_nested_type(&member.model_id, &member.ty, nested_members),
    }
}

fn parse_nested_type(
    target_id: &str,
    target_type: &str,
    nested_members: &[&ModelMember],
) -> TypeData {
    let nested_mapping: TypeMapping = nested_members
        .iter()
        .filter_map(|&member| {
            if target_id == member.model_id && member.id.ends_with(target_type) {
                let type_data = member_to_type_data(member, nested_members);
                Some((Name::new(&member.name), type_data))
            } else {
                None
            }
        })
        .collect();

    TypeData::Nested((TypeRef::named(target_type), nested_mapping))
}

pub fn value_mapping_from_row(
    row: &SqliteRow,
    types: &TypeMapping,
    is_external: bool,
) -> sqlx::Result<ValueMapping> {
    let mut value_mapping = types
        .iter()
        .filter(|(_, type_data)| type_data.is_simple())
        .map(|(field_name, type_data)| {
            let column_name = if is_external {
                format!("external_{}", field_name)
            } else {
                field_name.to_string()
            };

            Ok((
                Name::new(field_name),
                fetch_value(row, &column_name, &type_data.type_ref().to_string())?,
            ))
        })
        .collect::<sqlx::Result<ValueMapping>>()?;

    if let Ok(entity_id) = fetch_value(row, "entity_id", TypeRef::STRING) {
        value_mapping.insert(Name::new("entity_id"), entity_id);
    }

    Ok(value_mapping)
}

fn fetch_value(row: &SqliteRow, column_name: &str, field_type: &str) -> sqlx::Result<Value> {
    match Primitive::from_str(field_type) {
        // fetch boolean
        Ok(Primitive::Bool(_)) => {
            Ok(Value::from(matches!(row.try_get::<i64, &str>(column_name)?, BOOLEAN_TRUE)))
        }
        // fetch integer
        Ok(ty) if ty.to_sql_type() == "INTEGER" => {
            row.try_get::<i64, &str>(column_name).map(Value::from)
        }
        // fetch string
        _ => row.try_get::<String, &str>(column_name).map(Value::from),
    }
}
