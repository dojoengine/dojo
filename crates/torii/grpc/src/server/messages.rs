use std::str::FromStr;

use dojo_types::schema::Ty;
use sqlx::{sqlite::SqliteRow, Row};
use starknet_crypto::FieldElement;
use torii_core::{
    error::Error,
    error::{ParseError, QueryError},
    model::{build_sql_query, map_row_to_ty},
};

use crate::{proto, types::ComparisonOperator};

use super::DojoWorld;

impl DojoWorld {
    pub(crate) async fn messages_all(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Message>, Error> {
        self.messages_by_hashed_keys(None, limit, offset).await
    }

    pub(crate) async fn messages_by_hashed_keys(
        &self,
        hashed_keys: Option<proto::types::HashedKeysClause>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Message>, Error> {
        // TODO: use prepared statement for where clause
        let filter_ids = match hashed_keys {
            Some(hashed_keys) => {
                let ids = hashed_keys
                    .hashed_keys
                    .iter()
                    .map(|id| {
                        Ok(FieldElement::from_byte_slice_be(id)
                            .map(|id| format!("messages.id = '{id:#x}'"))
                            .map_err(ParseError::FromByteSliceError)?)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;

                format!("WHERE {}", ids.join(" OR "))
            }
            None => String::new(),
        };

        let query = format!(
            r#"
            SELECT messages.id, group_concat(message_model.model_id) as model_names
            FROM messages
            JOIN message_model ON messages.id = message_model.message_id
            {filter_ids}
            GROUP BY messages.id
            ORDER BY messages.event_id DESC
            LIMIT ? OFFSET ?
         "#
        );

        let db_messages: Vec<(String, String)> =
            sqlx::query_as(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let mut messages = Vec::with_capacity(db_messages.len());
        for (message_id, models_str) in db_messages {
            let model_names: Vec<&str> = models_str.split(',').collect();
            let schemas = self.model_cache.schemas(model_names).await?;

            let message_query = format!("{} WHERE messages.id = ?", build_sql_query(&schemas)?);
            let row = sqlx::query(&message_query).bind(&message_id).fetch_one(&self.pool).await?;

            let models = schemas
                .iter()
                .map(|s| {
                    let mut struct_ty = s.as_struct().expect("schema should be struct").to_owned();
                    map_row_to_ty(&s.name(), &mut struct_ty, &row)?;

                    Ok(struct_ty.try_into().unwrap())
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let hashed_keys = FieldElement::from_str(&message_id).map_err(ParseError::FromStr)?;
            let topic = row.get::<String, _>("topic");
            messages.push(proto::types::Message {
                hashed_keys: hashed_keys.to_bytes_be().to_vec(),
                topic,
                models,
            })
        }

        Ok(messages)
    }

    pub(crate) async fn messages_by_keys(
        &self,
        keys_clause: proto::types::KeysClause,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<proto::types::Message>, Error> {
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
            SELECT group_concat(message_model.model_id) as model_names
            FROM messages
            JOIN message_model ON messages.id = message_model.message_id
            WHERE messages.keys LIKE ?
            GROUP BY messages.id
            HAVING model_names REGEXP '(^|,){}(,|$)'
            LIMIT 1
        "#,
            keys_clause.model
        );
        let (models_str,): (String,) =
            sqlx::query_as(&models_query).bind(&keys_pattern).fetch_one(&self.pool).await?;

        let model_names = models_str.split(',').collect::<Vec<&str>>();
        let schemas = self.model_cache.schemas(model_names).await?;

        let messages_query = format!(
            "{} WHERE messages.keys LIKE ? ORDER BY messages.event_id DESC LIMIT ? OFFSET ?",
            build_sql_query(&schemas)?
        );
        let db_messages = sqlx::query(&messages_query)
            .bind(&keys_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        db_messages.iter().map(|row| Self::map_row_to_message(row, &schemas)).collect()
    }

    pub(crate) async fn messages_by_member(
        &self,
        member_clause: proto::types::MemberClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<proto::types::Message>, Error> {
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
            SELECT group_concat(message_model.model_id) as model_names
            FROM messages
            JOIN message_model ON messages.id = message_model.message_id
            GROUP BY messages.id
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

        let db_messages =
            sqlx::query(&member_query).bind(comparison_value).fetch_all(&self.pool).await?;

        db_messages.iter().map(|row| Self::map_row_to_message(row, &schemas)).collect()
    }

    pub(crate) async fn messages_by_composite(
        &self,
        _composite: proto::types::CompositeClause,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<proto::types::Message>, Error> {
        // TODO: Implement
        Err(QueryError::UnsupportedQuery.into())
    }

    fn map_row_to_message(row: &SqliteRow, schemas: &[Ty]) -> Result<proto::types::Message, Error> {
        let hashed_keys =
            FieldElement::from_str(&row.get::<String, _>("id")).map_err(ParseError::FromStr)?;
        let topic = row.get::<String, _>("topic");

        let models = schemas
            .iter()
            .map(|schema| {
                let mut struct_ty = schema.as_struct().expect("schema should be struct").to_owned();
                map_row_to_ty(&schema.name(), &mut struct_ty, row)?;

                Ok(struct_ty.try_into().unwrap())
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(proto::types::Message { hashed_keys: hashed_keys.to_bytes_be().to_vec(), topic, models })
    }
}
