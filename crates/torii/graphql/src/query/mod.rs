use async_graphql::dynamic::TypeRef;
use async_graphql::Name;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, QueryBuilder, Result, Sqlite};

use self::filter::{Filter, FilterValue};
use crate::object::model_data::ModelMember;
use crate::types::{TypeData, TypeMapping};

pub mod filter;
pub mod order;

pub async fn query_by_id<T>(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    id: &str,
) -> Result<T>
where
    T: Send + Unpin + for<'a> FromRow<'a, SqliteRow>,
{
    let query = format!("SELECT * FROM {} WHERE id = ?", table_name);
    let result = sqlx::query_as::<_, T>(&query).bind(id).fetch_one(conn).await?;

    Ok(result)
}

pub async fn query_all<T>(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    limit: i64,
) -> Result<Vec<T>>
where
    T: Send + Unpin + for<'a> FromRow<'a, SqliteRow>,
{
    let mut builder: QueryBuilder<'_, Sqlite> = QueryBuilder::new("SELECT * FROM ");
    builder.push(table_name).push(" ORDER BY created_at DESC LIMIT ").push(limit);
    let results: Vec<T> = builder.build_query_as().fetch_all(conn).await?;
    Ok(results)
}

pub async fn query_total_count(
    conn: &mut PoolConnection<Sqlite>,
    table_name: &str,
    filters: &Vec<Filter>,
) -> Result<i64> {
    let mut query = format!("SELECT COUNT(*) FROM {}", table_name);
    let mut conditions = Vec::new();

    for filter in filters {
        let condition = match filter.value {
            FilterValue::Int(i) => format!("{} {} {}", filter.field, filter.comparator, i),
            FilterValue::String(ref s) => format!("{} {} '{}'", filter.field, filter.comparator, s),
        };

        conditions.push(condition);
    }

    if !conditions.is_empty() {
        query.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    let result: (i64,) = sqlx::query_as(&query).fetch_one(conn).await?;
    Ok(result.0)
}

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
