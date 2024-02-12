use std::str::FromStr;

use dojo_types::schema::Ty;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use starknet_crypto::FieldElement;
use torii_core::error::{Error, ParseError, QueryError};
use torii_core::model::{build_sql_query, map_row_to_ty};

use super::DojoWorld;
use crate::proto;
use crate::types::ComparisonOperator;

impl DojoWorld {
    pub(crate) async fn entities_all(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        self.entities_by_hashed_keys(None, limit, offset).await
    }

    pub(crate) async fn entities_by_hashed_keys(
        &self,
        hashed_keys: Option<proto::types::HashedKeysClause>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        // TODO: use prepared statement for where clause
        let filter_ids = match hashed_keys {
            Some(hashed_keys) => {
                let ids = hashed_keys
                    .hashed_keys
                    .iter()
                    .map(|id| {
                        Ok(FieldElement::from_byte_slice_be(id)
                            .map(|id| format!("entities.id = '{id:#x}'"))
                            .map_err(ParseError::FromByteSliceError)?)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;

                format!("WHERE {}", ids.join(" OR "))
            }
            None => String::new(),
        };

        let query = format!(
            r#"
            SELECT entities.id, group_concat(entity_model.model_id) as model_names
            FROM entities
            JOIN entity_model ON entities.id = entity_model.entity_id
            {filter_ids}
            GROUP BY entities.id
            ORDER BY entities.event_id DESC
            LIMIT ? OFFSET ?
         "#
        );

        let db_entities: Vec<(String, String)> =
            sqlx::query_as(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let mut entities = Vec::with_capacity(db_entities.len());
        for (entity_id, models_str) in db_entities {
            let model_names: Vec<&str> = models_str.split(',').collect();
            let schemas = self.model_cache.schemas(model_names).await?;

            let entity_query = format!("{} WHERE entities.id = ?", build_sql_query(&schemas)?);
            let row = sqlx::query(&entity_query).bind(&entity_id).fetch_one(&self.pool).await?;

            let models = schemas
                .iter()
                .map(|s| {
                    let mut struct_ty = s.as_struct().expect("schema should be struct").to_owned();
                    map_row_to_ty(&s.name(), &mut struct_ty, &row)?;

                    Ok(struct_ty.try_into().unwrap())
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let hashed_keys = FieldElement::from_str(&entity_id).map_err(ParseError::FromStr)?;
            entities.push(proto::types::Entity {
                hashed_keys: hashed_keys.to_bytes_be().to_vec(),
                models,
            })
        }

        Ok(entities)
    }

    pub(crate) async fn entities_by_keys(
        &self,
        keys_clause: proto::types::KeysClause,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        let keys = keys_clause
            .keys
            .iter()
            .map(|bytes| {
                if bytes.is_empty() {
                    return Ok("%".to_string());
                }
                Ok(FieldElement::from_byte_slice_be(bytes)
                    .map(|felt| format!("{:#x}", felt))
                    .map_err(ParseError::FromByteSliceError)?)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let keys_pattern = keys.join("/") + "/%";

        let models_query = format!(
            r#"
            SELECT group_concat(entity_model.model_id) as model_names
            FROM entities
            JOIN entity_model ON entities.id = entity_model.entity_id
            WHERE entities.keys LIKE ?
            GROUP BY entities.id
            HAVING model_names REGEXP '(^|,){}(,|$)'
            LIMIT 1
        "#,
            keys_clause.model
        );
        let (models_str,): (String,) =
            sqlx::query_as(&models_query).bind(&keys_pattern).fetch_one(&self.pool).await?;

        let model_names = models_str.split(',').collect::<Vec<&str>>();
        let schemas = self.model_cache.schemas(model_names).await?;

        let entities_query = format!(
            "{} WHERE entities.keys LIKE ? ORDER BY entities.event_id DESC LIMIT ? OFFSET ?",
            build_sql_query(&schemas)?
        );
        let db_entities = sqlx::query(&entities_query)
            .bind(&keys_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        db_entities.iter().map(|row| Self::map_row_to_entity(row, &schemas)).collect()
    }

    pub(crate) async fn entities_by_member(
        &self,
        member_clause: proto::types::MemberClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        let comparison_operator = ComparisonOperator::from_repr(member_clause.operator as usize)
            .expect("invalid comparison operator");

        let value_type = member_clause
            .value
            .ok_or(QueryError::MissingParam("value".into()))?
            .value_type
            .ok_or(QueryError::MissingParam("value_type".into()))?;

        let comparison_value = match value_type {
            proto::types::value::ValueType::StringValue(string) => string,
            proto::types::value::ValueType::IntValue(int) => int.to_string(),
            proto::types::value::ValueType::UintValue(uint) => uint.to_string(),
            proto::types::value::ValueType::BoolValue(bool) => {
                if bool {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            _ => return Err(QueryError::UnsupportedQuery.into()),
        };

        let models_query = format!(
            r#"
            SELECT group_concat(entity_model.model_id) as model_names
            FROM entities
            JOIN entity_model ON entities.id = entity_model.entity_id
            GROUP BY entities.id
            HAVING model_names REGEXP '(^|,){}(,|$)'
            LIMIT 1
        "#,
            member_clause.model
        );
        let (models_str,): (String,) = sqlx::query_as(&models_query).fetch_one(&self.pool).await?;

        let model_names = models_str.split(',').collect::<Vec<&str>>();
        let schemas = self.model_cache.schemas(model_names).await?;

        let table_name = member_clause.model;
        let column_name = format!("external_{}", member_clause.member);
        let member_query = format!(
            "{} WHERE {table_name}.{column_name} {comparison_operator} ?",
            build_sql_query(&schemas)?
        );

        let db_entities =
            sqlx::query(&member_query).bind(comparison_value).fetch_all(&self.pool).await?;

        db_entities.iter().map(|row| Self::map_row_to_entity(row, &schemas)).collect()
    }

    pub(crate) async fn entities_by_composite(
        &self,
        _composite: proto::types::CompositeClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        // TODO: Implement
        Err(QueryError::UnsupportedQuery.into())
    }

    fn map_row_to_entity(row: &SqliteRow, schemas: &[Ty]) -> Result<proto::types::Entity, Error> {
        let hashed_keys =
            FieldElement::from_str(&row.get::<String, _>("id")).map_err(ParseError::FromStr)?;
        let models = schemas
            .iter()
            .map(|schema| {
                let mut struct_ty = schema.as_struct().expect("schema should be struct").to_owned();
                map_row_to_ty(&schema.name(), &mut struct_ty, row)?;

                Ok(struct_ty.try_into().unwrap())
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(proto::types::Entity { hashed_keys: hashed_keys.to_bytes_be().to_vec(), models })
    }
}
