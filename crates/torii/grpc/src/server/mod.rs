pub mod logger;
pub mod subscriptions;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str;
use std::str::FromStr;
use std::sync::Arc;

use dojo_types::primitive::{Primitive, PrimitiveError};
use dojo_types::schema::Ty;
use dojo_world::contracts::naming::compute_selector_from_names;
use futures::Stream;
use proto::world::{
    RetrieveEntitiesRequest, RetrieveEntitiesResponse, RetrieveEventsRequest,
    RetrieveEventsResponse, SubscribeModelsRequest, SubscribeModelsResponse,
    UpdateEntitiesSubscriptionRequest,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlx::prelude::FromRow;
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::types::Felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use subscriptions::event::EventManager;
use subscriptions::indexer::IndexerManager;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use torii_core::error::{Error, ParseError, QueryError};
use torii_core::model::{build_sql_query, map_row_to_ty};
use torii_core::sql::cache::ModelCache;

use self::subscriptions::entity::EntityManager;
use self::subscriptions::event_message::EventMessageManager;
use self::subscriptions::model_diff::{ModelDiffRequest, StateDiffManager};
use crate::proto::types::clause::ClauseType;
use crate::proto::types::member_value::ValueType;
use crate::proto::types::LogicalOperator;
use crate::proto::world::world_server::WorldServer;
use crate::proto::world::{
    SubscribeEntitiesRequest, SubscribeEntityResponse, SubscribeEventsResponse,
    SubscribeIndexerRequest, SubscribeIndexerResponse, WorldMetadataRequest, WorldMetadataResponse,
};
use crate::proto::{self};
use crate::types::schema::SchemaError;
use crate::types::ComparisonOperator;

pub(crate) static ENTITIES_TABLE: &str = "entities";
pub(crate) static ENTITIES_MODEL_RELATION_TABLE: &str = "entity_model";
pub(crate) static ENTITIES_ENTITY_RELATION_COLUMN: &str = "entity_id";

pub(crate) static EVENT_MESSAGES_TABLE: &str = "event_messages";
pub(crate) static EVENT_MESSAGES_MODEL_RELATION_TABLE: &str = "event_model";
pub(crate) static EVENT_MESSAGES_ENTITY_RELATION_COLUMN: &str = "event_message_id";

impl From<SchemaError> for Error {
    fn from(err: SchemaError) -> Self {
        match err {
            SchemaError::MissingExpectedData(data) => QueryError::MissingParam(data).into(),
            SchemaError::UnsupportedType(data) => QueryError::UnsupportedValue(data).into(),
            SchemaError::InvalidByteLength(got, expected) => {
                PrimitiveError::InvalidByteLength(got, expected).into()
            }
            SchemaError::ParseIntError(err) => ParseError::ParseIntError(err).into(),
            SchemaError::FromSlice(err) => ParseError::FromSlice(err).into(),
            SchemaError::FromStr(err) => ParseError::FromStr(err).into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DojoWorld {
    pool: Pool<Sqlite>,
    world_address: Felt,
    model_cache: Arc<ModelCache>,
    entity_manager: Arc<EntityManager>,
    event_message_manager: Arc<EventMessageManager>,
    event_manager: Arc<EventManager>,
    state_diff_manager: Arc<StateDiffManager>,
    indexer_manager: Arc<IndexerManager>,
}

impl DojoWorld {
    pub fn new(
        pool: Pool<Sqlite>,
        block_rx: Receiver<u64>,
        world_address: Felt,
        provider: Arc<JsonRpcClient<HttpTransport>>,
    ) -> Self {
        let model_cache = Arc::new(ModelCache::new(pool.clone()));
        let entity_manager = Arc::new(EntityManager::default());
        let event_message_manager = Arc::new(EventMessageManager::default());
        let event_manager = Arc::new(EventManager::default());
        let state_diff_manager = Arc::new(StateDiffManager::default());
        let indexer_manager = Arc::new(IndexerManager::default());

        tokio::task::spawn(subscriptions::model_diff::Service::new_with_block_rcv(
            block_rx,
            world_address,
            provider,
            Arc::clone(&state_diff_manager),
        ));

        tokio::task::spawn(subscriptions::entity::Service::new(Arc::clone(&entity_manager)));

        tokio::task::spawn(subscriptions::event_message::Service::new(Arc::clone(
            &event_message_manager,
        )));

        tokio::task::spawn(subscriptions::event::Service::new(Arc::clone(&event_manager)));

        tokio::task::spawn(subscriptions::indexer::Service::new(Arc::clone(&indexer_manager)));

        Self {
            pool,
            world_address,
            model_cache,
            entity_manager,
            event_message_manager,
            event_manager,
            state_diff_manager,
            indexer_manager,
        }
    }
}

impl DojoWorld {
    pub async fn world(&self) -> Result<proto::types::WorldMetadata, Error> {
        let world_address = sqlx::query_scalar(&format!(
            "SELECT contract_address FROM contracts WHERE id = '{:#x}'",
            self.world_address
        ))
        .fetch_one(&self.pool)
        .await?;

        #[derive(FromRow)]
        struct ModelDb {
            id: String,
            namespace: String,
            name: String,
            class_hash: String,
            contract_address: String,
            packed_size: u32,
            unpacked_size: u32,
            layout: String,
        }

        let models: Vec<ModelDb> = sqlx::query_as(
            "SELECT id, namespace, name, class_hash, contract_address, packed_size, \
             unpacked_size, layout FROM models",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut models_metadata = Vec::with_capacity(models.len());
        for model in models {
            let schema = self
                .model_cache
                .model(&Felt::from_str(&model.id).map_err(ParseError::FromStr)?)
                .await?
                .schema;
            models_metadata.push(proto::types::ModelMetadata {
                namespace: model.namespace,
                name: model.name,
                class_hash: model.class_hash,
                contract_address: model.contract_address,
                packed_size: model.packed_size,
                unpacked_size: model.unpacked_size,
                layout: model.layout.as_bytes().to_vec(),
                schema: serde_json::to_vec(&schema).unwrap(),
            });
        }

        Ok(proto::types::WorldMetadata { world_address, models: models_metadata })
    }

    async fn entities_all(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        self.query_by_hashed_keys(
            table,
            model_relation_table,
            entity_relation_column,
            None,
            Some(limit),
            Some(offset),
        )
        .await
    }

    async fn events_all(&self, limit: u32, offset: u32) -> Result<Vec<proto::types::Event>, Error> {
        let query = r#"
            SELECT keys, data, transaction_hash
            FROM events
            ORDER BY id DESC
            LIMIT ? OFFSET ?
         "#
        .to_string();

        let row_events: Vec<(String, String, String)> =
            sqlx::query_as(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;
        row_events.iter().map(map_row_to_event).collect()
    }

    async fn fetch_entities(
        &self,
        table: &str,
        entity_relation_column: &str,
        entities: Vec<(String, String)>,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        // Group entities by their model combinations
        let mut model_groups: HashMap<String, Vec<String>> = HashMap::new();
        for (entity_id, models_str) in entities {
            model_groups.entry(models_str).or_default().push(entity_id);
        }

        let mut all_entities = Vec::new();

        let mut tx = self.pool.begin().await?;

        // Create a temporary table to store entity IDs due to them potentially exceeding
        // SQLite's parameters limit which is 999
        sqlx::query(
            "CREATE TEMPORARY TABLE temp_entity_ids (id TEXT PRIMARY KEY, model_group TEXT)",
        )
        .execute(&mut *tx)
        .await?;

        // Insert all entity IDs into the temporary table
        for (model_ids, entity_ids) in &model_groups {
            for chunk in entity_ids.chunks(999) {
                let placeholders = chunk.iter().map(|_| "(?, ?)").collect::<Vec<_>>().join(",");
                let query = format!(
                    "INSERT INTO temp_entity_ids (id, model_group) VALUES {}",
                    placeholders
                );
                let mut query = sqlx::query(&query);
                for id in chunk {
                    query = query.bind(id).bind(model_ids);
                }
                query.execute(&mut *tx).await?;
            }
        }

        for (models_str, _) in model_groups {
            let model_ids =
                models_str.split(',').map(|id| Felt::from_str(id).unwrap()).collect::<Vec<_>>();
            let schemas =
                self.model_cache.models(&model_ids).await?.into_iter().map(|m| m.schema).collect();

            let (entity_query, arrays_queries, _) = build_sql_query(
                &schemas,
                table,
                entity_relation_column,
                Some(&format!(
                    "[{table}].id IN (SELECT id FROM temp_entity_ids WHERE model_group = ?)"
                )),
                Some(&format!(
                    "[{table}].id IN (SELECT id FROM temp_entity_ids WHERE model_group = ?)"
                )),
                None,
                None,
            )?;

            let rows = sqlx::query(&entity_query).bind(&models_str).fetch_all(&mut *tx).await?;

            let mut arrays_rows = HashMap::new();
            for (name, array_query) in arrays_queries {
                let array_rows =
                    sqlx::query(&array_query).bind(&models_str).fetch_all(&mut *tx).await?;
                arrays_rows.insert(name, array_rows);
            }

            let arrays_rows = Arc::new(arrays_rows);
            let schemas = Arc::new(schemas);

            let group_entities: Result<Vec<_>, Error> = rows
                .par_iter()
                .map(|row| map_row_to_entity(row, &arrays_rows, (*schemas).clone()))
                .collect();

            all_entities.extend(group_entities?);
        }

        sqlx::query("DROP TABLE temp_entity_ids").execute(&mut *tx).await?;

        tx.commit().await?;

        Ok(all_entities)
    }

    pub(crate) async fn query_by_hashed_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        hashed_keys: Option<proto::types::HashedKeysClause>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        // TODO: use prepared statement for where clause
        let filter_ids = match hashed_keys {
            Some(hashed_keys) => {
                let ids = hashed_keys
                    .hashed_keys
                    .iter()
                    .map(|id| Ok(format!("{table}.id = '{:#x}'", Felt::from_bytes_be_slice(id))))
                    .collect::<Result<Vec<_>, Error>>()?;

                format!("WHERE {}", ids.join(" OR "))
            }
            None => String::new(),
        };

        // count query that matches filter_ids
        let count_query = format!(
            r#"
                    SELECT count(*)
                    FROM {table}
                    {filter_ids}
                "#
        );
        // total count of rows without limit and offset
        let total_count: u32 =
            sqlx::query_scalar(&count_query).fetch_optional(&self.pool).await?.unwrap_or(0);
        if total_count == 0 {
            return Ok((Vec::new(), 0));
        }

        // Query to get entity IDs and their model IDs
        let mut query = format!(
            r#"
            SELECT {table}.id, group_concat({model_relation_table}.model_id) as model_ids
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            {filter_ids}
            GROUP BY {table}.id
            ORDER BY {table}.event_id DESC
         "#
        );

        if limit.is_some() {
            query += " LIMIT ?"
        }

        if offset.is_some() {
            query += " OFFSET ?"
        }

        let db_entities: Vec<(String, String)> =
            sqlx::query_as(&query).bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let entities = self.fetch_entities(table, entity_relation_column, db_entities).await?;
        Ok((entities, total_count))
    }

    pub(crate) async fn query_by_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        keys_clause: &proto::types::KeysClause,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let keys_pattern = build_keys_pattern(keys_clause)?;

        // total count of rows that matches keys_pattern without limit and offset
        let count_query = format!(
            r#"
            SELECT count(*)
            FROM {table}
            {}
        "#,
            if !keys_clause.models.is_empty() {
                // split the model names to namespace and model
                let model_ids = keys_clause
                    .models
                    .iter()
                    .map(|model| {
                        model
                            .split_once('-')
                            .ok_or(QueryError::InvalidNamespacedModel(model.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                // get the model selector from namespace and model and format
                let model_ids_str = model_ids
                    .iter()
                    .map(|(namespace, model)| {
                        format!("'{:#x}'", compute_selector_from_names(namespace, model))
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    r#"
                JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
                WHERE {model_relation_table}.model_id IN ({})
                AND {table}.keys REGEXP ?
            "#,
                    model_ids_str
                )
            } else {
                format!(
                    r#"
                WHERE {table}.keys REGEXP ?
            "#
                )
            }
        );

        let total_count = sqlx::query_scalar(&count_query)
            .bind(&keys_pattern)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);
        if total_count == 0 {
            return Ok((Vec::new(), 0));
        }

        let mut models_query = format!(
            r#"
            SELECT {table}.id, group_concat({model_relation_table}.model_id) as model_ids
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            WHERE {table}.keys REGEXP ?
            GROUP BY {table}.id
        "#
        );

        if !keys_clause.models.is_empty() {
            // filter by models
            models_query += &format!(
                "HAVING {}",
                keys_clause
                    .models
                    .iter()
                    .map(|model| {
                        let (namespace, name) = model
                            .split_once('-')
                            .ok_or(QueryError::InvalidNamespacedModel(model.clone()))?;
                        let model_id = compute_selector_from_names(namespace, name);
                        Ok(format!("INSTR(model_ids, '{:#x}') > 0", model_id))
                    })
                    .collect::<Result<Vec<_>, Error>>()?
                    .join(" OR ")
                    .as_str()
            );
        }

        models_query += &format!(" ORDER BY {table}.event_id DESC");

        if limit.is_some() {
            models_query += " LIMIT ?";
        }
        if offset.is_some() {
            models_query += " OFFSET ?";
        }

        let db_entities: Vec<(String, String)> = sqlx::query_as(&models_query)
            .bind(&keys_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let entities = self.fetch_entities(table, entity_relation_column, db_entities).await?;
        Ok((entities, total_count))
    }

    pub(crate) async fn events_by_keys(
        &self,
        keys_clause: &proto::types::KeysClause,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<proto::types::Event>, Error> {
        let keys_pattern = build_keys_pattern(keys_clause)?;

        let events_query = r#"
            SELECT keys, data, transaction_hash
            FROM events
            WHERE keys REGEXP ?
            ORDER BY id DESC
            LIMIT ? OFFSET ?
        "#
        .to_string();

        let row_events: Vec<(String, String, String)> = sqlx::query_as(&events_query)
            .bind(&keys_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        row_events.iter().map(map_row_to_event).collect()
    }

    pub(crate) async fn query_by_member(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        member_clause: proto::types::MemberClause,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let comparison_operator = ComparisonOperator::from_repr(member_clause.operator as usize)
            .expect("invalid comparison operator");

        let comparison_value =
            match member_clause.value.ok_or(QueryError::MissingParam("value".into()))?.value_type {
                Some(ValueType::String(value)) => value,
                Some(ValueType::Primitive(value)) => {
                    let primitive: Primitive = value.try_into()?;
                    primitive.to_sql_value()?
                }
                None => return Err(QueryError::MissingParam("value_type".into()).into()),
            };

        let (namespace, model) = member_clause
            .model
            .split_once('-')
            .ok_or(QueryError::InvalidNamespacedModel(member_clause.model.clone()))?;

        let models_query = format!(
            r#"
            SELECT group_concat({model_relation_table}.model_id) as model_ids
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            GROUP BY {table}.id
            HAVING INSTR(model_ids, '{:#x}') > 0
            LIMIT 1
        "#,
            compute_selector_from_names(namespace, model)
        );
        let models_str: Option<String> =
            sqlx::query_scalar(&models_query).fetch_optional(&self.pool).await?;
        if models_str.is_none() {
            return Ok((Vec::new(), 0));
        }

        let models_str = models_str.unwrap();

        let model_ids = models_str
            .split(',')
            .map(Felt::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ParseError::FromStr)?;
        let schemas =
            self.model_cache.models(&model_ids).await?.into_iter().map(|m| m.schema).collect();

        let model = member_clause.model.clone();
        let parts: Vec<&str> = member_clause.member.split('.').collect();
        let (table_name, column_name) = if parts.len() > 1 {
            let nested_table = parts[..parts.len() - 1].join("$");
            (format!("{model}${nested_table}"), format!("external_{}", parts.last().unwrap()))
        } else {
            (model, format!("external_{}", member_clause.member))
        };
        let (entity_query, arrays_queries, count_query) = build_sql_query(
            &schemas,
            table,
            entity_relation_column,
            Some(&format!("[{table_name}].{column_name} {comparison_operator} ?")),
            None,
            limit,
            offset,
        )?;

        let total_count = sqlx::query_scalar(&count_query)
            .bind(comparison_value.clone())
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0);
        let db_entities = sqlx::query(&entity_query)
            .bind(comparison_value.clone())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut arrays_rows = HashMap::new();
        for (name, query) in arrays_queries {
            let rows =
                sqlx::query(&query).bind(comparison_value.clone()).fetch_all(&self.pool).await?;
            arrays_rows.insert(name, rows);
        }

        let arrays_rows = Arc::new(arrays_rows);
        let entities_collection: Result<Vec<_>, Error> = db_entities
            .par_iter()
            .map(|row| {
                let schemas_clone = schemas.clone();
                let arrays_rows_clone = arrays_rows.clone();
                map_row_to_entity(row, &arrays_rows_clone, schemas_clone)
            })
            .collect();
        Ok((entities_collection?, total_count))
    }

    pub(crate) async fn query_by_composite(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        composite: proto::types::CompositeClause,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let (where_clause, having_clause, join_clause, bind_values) =
            build_composite_clause(table, model_relation_table, &composite)?;

        let count_query = format!(
            r#"
            SELECT COUNT(DISTINCT [{table}].id)
            FROM [{table}]
            JOIN {model_relation_table} ON [{table}].id = {model_relation_table}.entity_id
            {join_clause}
            {where_clause}
            {having_clause}
            "#
        );

        let mut count_query = sqlx::query_scalar::<_, u32>(&count_query);
        for value in &bind_values {
            count_query = count_query.bind(value);
        }

        let total_count = count_query.fetch_optional(&self.pool).await?.unwrap_or(0);
        if total_count == 0 {
            return Ok((Vec::new(), 0));
        }

        let query = format!(
            r#"
            SELECT [{table}].id, group_concat({model_relation_table}.model_id) as model_ids
            FROM [{table}]
            JOIN {model_relation_table} ON [{table}].id = {model_relation_table}.entity_id
            {join_clause}
            {where_clause}
            GROUP BY [{table}].id
            {having_clause}
            ORDER BY [{table}].event_id DESC
            LIMIT ? OFFSET ?
            "#
        );

        let mut db_query = sqlx::query_as(&query);
        for value in &bind_values {
            db_query = db_query.bind(value);
        }
        db_query = db_query.bind(limit.unwrap_or(u32::MAX)).bind(offset.unwrap_or(0));

        let db_entities: Vec<(String, String)> = db_query.fetch_all(&self.pool).await?;

        let entities = self.fetch_entities(table, entity_relation_column, db_entities).await?;
        Ok((entities, total_count))
    }

    pub async fn model_metadata(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<proto::types::ModelMetadata, Error> {
        // selector
        let model = compute_selector_from_names(namespace, name);

        let model = self.model_cache.model(&model).await?;

        Ok(proto::types::ModelMetadata {
            namespace: namespace.to_string(),
            name: name.to_string(),
            class_hash: format!("{:#x}", model.class_hash),
            contract_address: format!("{:#x}", model.contract_address),
            packed_size: model.packed_size,
            unpacked_size: model.unpacked_size,
            layout: serde_json::to_vec(&model.layout).unwrap(),
            schema: serde_json::to_vec(&model.schema).unwrap(),
        })
    }

    async fn subscribe_indexer(
        &self,
        contract_address: Felt,
    ) -> Result<Receiver<Result<proto::world::SubscribeIndexerResponse, tonic::Status>>, Error>
    {
        self.indexer_manager.add_subscriber(&self.pool, contract_address).await
    }

    async fn subscribe_models(
        &self,
        models_keys: Vec<proto::types::ModelKeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeModelsResponse, tonic::Status>>, Error> {
        let mut subs = Vec::with_capacity(models_keys.len());
        for keys in models_keys {
            let (namespace, model) = keys
                .model
                .split_once('-')
                .ok_or(QueryError::InvalidNamespacedModel(keys.model.clone()))?;

            let selector = compute_selector_from_names(namespace, model);

            let proto::types::ModelMetadata { packed_size, .. } =
                self.model_metadata(namespace, model).await?;

            subs.push(ModelDiffRequest {
                keys,
                model: subscriptions::model_diff::ModelMetadata {
                    selector,
                    packed_size: packed_size as usize,
                },
            });
        }

        self.state_diff_manager.add_subscriber(subs).await
    }

    async fn subscribe_entities(
        &self,
        keys: Vec<proto::types::EntityKeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        self.entity_manager.add_subscriber(keys.into_iter().map(|keys| keys.into()).collect()).await
    }

    async fn retrieve_entities(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        query: proto::types::Query,
    ) -> Result<proto::world::RetrieveEntitiesResponse, Error> {
        let (entities, total_count) = match query.clause {
            None => {
                self.entities_all(
                    table,
                    model_relation_table,
                    entity_relation_column,
                    query.limit,
                    query.offset,
                )
                .await?
            }
            Some(clause) => {
                let clause_type =
                    clause.clause_type.ok_or(QueryError::MissingParam("clause_type".into()))?;

                match clause_type {
                    ClauseType::HashedKeys(hashed_keys) => {
                        self.query_by_hashed_keys(
                            table,
                            model_relation_table,
                            entity_relation_column,
                            if hashed_keys.hashed_keys.is_empty() {
                                None
                            } else {
                                Some(hashed_keys)
                            },
                            Some(query.limit),
                            Some(query.offset),
                        )
                        .await?
                    }
                    ClauseType::Keys(keys) => {
                        self.query_by_keys(
                            table,
                            model_relation_table,
                            entity_relation_column,
                            &keys,
                            Some(query.limit),
                            Some(query.offset),
                        )
                        .await?
                    }
                    ClauseType::Member(member) => {
                        self.query_by_member(
                            table,
                            model_relation_table,
                            entity_relation_column,
                            member,
                            Some(query.limit),
                            Some(query.offset),
                        )
                        .await?
                    }
                    ClauseType::Composite(composite) => {
                        self.query_by_composite(
                            table,
                            model_relation_table,
                            entity_relation_column,
                            composite,
                            Some(query.limit),
                            Some(query.offset),
                        )
                        .await?
                    }
                }
            }
        };

        Ok(RetrieveEntitiesResponse { entities, total_count })
    }

    async fn subscribe_event_messages(
        &self,
        clauses: Vec<proto::types::EntityKeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        self.event_message_manager
            .add_subscriber(clauses.into_iter().map(|keys| keys.into()).collect())
            .await
    }

    async fn retrieve_events(
        &self,
        query: &proto::types::EventQuery,
    ) -> Result<proto::world::RetrieveEventsResponse, Error> {
        let events = match &query.keys {
            None => self.events_all(query.limit, query.offset).await?,
            Some(keys) => self.events_by_keys(keys, Some(query.limit), Some(query.offset)).await?,
        };
        Ok(RetrieveEventsResponse { events })
    }

    async fn subscribe_events(
        &self,
        clause: Vec<proto::types::EntityKeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEventsResponse, tonic::Status>>, Error> {
        self.event_manager
            .add_subscriber(clause.into_iter().map(|keys| keys.into()).collect())
            .await
    }
}

fn process_event_field(data: &str) -> Result<Vec<Vec<u8>>, Error> {
    Ok(data
        .trim_end_matches('/')
        .split('/')
        .map(|d| Felt::from_str(d).map_err(ParseError::FromStr).map(|f| f.to_bytes_be().to_vec()))
        .collect::<Result<Vec<_>, _>>()?)
}

fn map_row_to_event(row: &(String, String, String)) -> Result<proto::types::Event, Error> {
    let keys = process_event_field(&row.0)?;
    let data = process_event_field(&row.1)?;
    let transaction_hash =
        Felt::from_str(&row.2).map_err(ParseError::FromStr)?.to_bytes_be().to_vec();

    Ok(proto::types::Event { keys, data, transaction_hash })
}

fn map_row_to_entity(
    row: &SqliteRow,
    arrays_rows: &HashMap<String, Vec<SqliteRow>>,
    mut schemas: Vec<Ty>,
) -> Result<proto::types::Entity, Error> {
    let hashed_keys = Felt::from_str(&row.get::<String, _>("id")).map_err(ParseError::FromStr)?;
    let models = schemas
        .iter_mut()
        .map(|schema| {
            map_row_to_ty("", &schema.name(), schema, row, arrays_rows)?;
            Ok(schema.as_struct().unwrap().clone().into())
        })
        .collect::<Result<Vec<_>, Error>>()?;

    Ok(proto::types::Entity { hashed_keys: hashed_keys.to_bytes_be().to_vec(), models })
}

// this builds a sql safe regex pattern to match against for keys
fn build_keys_pattern(clause: &proto::types::KeysClause) -> Result<String, Error> {
    const KEY_PATTERN: &str = "0x[0-9a-fA-F]+";

    let keys = if clause.keys.is_empty() {
        vec![KEY_PATTERN.to_string()]
    } else {
        clause
            .keys
            .iter()
            .map(|bytes| {
                if bytes.is_empty() {
                    return Ok(KEY_PATTERN.to_string());
                }
                Ok(format!("{:#x}", Felt::from_bytes_be_slice(bytes)))
            })
            .collect::<Result<Vec<_>, Error>>()?
    };
    let mut keys_pattern = format!("^{}", keys.join("/"));

    if clause.pattern_matching == proto::types::PatternMatching::VariableLen as i32 {
        keys_pattern += &format!("({})*", KEY_PATTERN);
    }
    keys_pattern += "/$";

    Ok(keys_pattern)
}

// builds a composite clause for a query
fn build_composite_clause(
    table: &str,
    model_relation_table: &str,
    composite: &proto::types::CompositeClause,
) -> Result<(String, String, String, Vec<String>), Error> {
    let is_or = composite.operator == LogicalOperator::Or as i32;
    let mut where_clauses = Vec::new();
    let mut join_clauses = Vec::new();
    let mut having_clauses = Vec::new();
    let mut bind_values = Vec::new();

    // HashMap to track the number of joins per model
    let mut model_counters: HashMap<String, usize> = HashMap::new();

    for clause in &composite.clauses {
        match clause.clause_type.as_ref().unwrap() {
            ClauseType::HashedKeys(hashed_keys) => {
                let ids = hashed_keys
                    .hashed_keys
                    .iter()
                    .map(|id| {
                        bind_values.push(Felt::from_bytes_be_slice(id).to_string());
                        "?".to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                where_clauses.push(format!("{table}.id IN ({})", ids));
            }
            ClauseType::Keys(keys) => {
                let keys_pattern = build_keys_pattern(keys)?;
                bind_values.push(keys_pattern);
                where_clauses.push(format!("{table}.keys REGEXP ?"));
            }
            ClauseType::Member(member) => {
                let comparison_operator = ComparisonOperator::from_repr(member.operator as usize)
                    .expect("invalid comparison operator");
                let value = member.value.clone();
                let comparison_value =
                    match value.ok_or(QueryError::MissingParam("value".into()))?.value_type {
                        Some(ValueType::String(value)) => value,
                        Some(ValueType::Primitive(value)) => {
                            let primitive: Primitive = value.try_into()?;
                            primitive.to_sql_value()?
                        }
                        None => return Err(QueryError::MissingParam("value_type".into()).into()),
                    };
                bind_values.push(comparison_value);

                let model = member.model.clone();
                let parts: Vec<&str> = member.member.split('.').collect();
                let (table_name, column_name) = if parts.len() > 1 {
                    let nested_table = parts[..parts.len() - 1].join("$");
                    (
                        format!("[{model}${nested_table}]"),
                        format!("external_{}", parts.last().unwrap()),
                    )
                } else {
                    (format!("[{model}]"), format!("external_{}", member.member))
                };

                let (namespace, model_name) = member
                    .model
                    .split_once('-')
                    .ok_or(QueryError::InvalidNamespacedModel(member.model.clone()))?;
                let model_id = compute_selector_from_names(namespace, model_name);

                // Generate a unique alias for each model
                let counter = model_counters.entry(model.clone()).or_insert(0);
                *counter += 1;
                let alias =
                    if *counter == 1 { model.clone() } else { format!("{model}_{}", *counter - 1) };

                join_clauses.push(format!(
                    "LEFT JOIN {table_name} AS [{alias}] ON [{table}].id = [{alias}].entity_id"
                ));
                where_clauses.push(format!("[{alias}].{column_name} {comparison_operator} ?"));
                having_clauses.push(format!(
                    "INSTR(group_concat({model_relation_table}.model_id), '{:#x}') > 0",
                    model_id
                ));
            }
            ClauseType::Composite(nested_composite) => {
                let (nested_where, nested_having, nested_join, nested_values) =
                    build_composite_clause(table, model_relation_table, nested_composite)?;
                where_clauses.push(format!("({})", nested_where.trim_start_matches("WHERE ")));
                if !nested_having.is_empty() {
                    having_clauses.push(nested_having.trim_start_matches("HAVING ").to_string());
                }
                join_clauses.extend(
                    nested_join
                        .split_whitespace()
                        .filter(|&s| s.starts_with("LEFT"))
                        .map(String::from),
                );
                bind_values.extend(nested_values);
            }
        }
    }

    let join_clause = join_clauses.join(" ");
    let where_clause = if !where_clauses.is_empty() {
        format!("WHERE {}", where_clauses.join(if is_or { " OR " } else { " AND " }))
    } else {
        String::new()
    };
    let having_clause = if !having_clauses.is_empty() {
        format!("HAVING {}", having_clauses.join(if is_or { " OR " } else { " AND " }))
    } else {
        String::new()
    };

    Ok((where_clause, having_clause, join_clause, bind_values))
}

type ServiceResult<T> = Result<Response<T>, Status>;
type SubscribeModelsResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeModelsResponse, Status>> + Send>>;
type SubscribeEntitiesResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeEntityResponse, Status>> + Send>>;
type SubscribeEventsResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeEventsResponse, Status>> + Send>>;
type SubscribeIndexerResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeIndexerResponse, Status>> + Send>>;

#[tonic::async_trait]
impl proto::world::world_server::World for DojoWorld {
    type SubscribeModelsStream = SubscribeModelsResponseStream;
    type SubscribeEntitiesStream = SubscribeEntitiesResponseStream;
    type SubscribeEventMessagesStream = SubscribeEntitiesResponseStream;
    type SubscribeEventsStream = SubscribeEventsResponseStream;
    type SubscribeIndexerStream = SubscribeIndexerResponseStream;

    async fn world_metadata(
        &self,
        _request: Request<WorldMetadataRequest>,
    ) -> Result<Response<WorldMetadataResponse>, Status> {
        let metadata = Some(self.world().await.map_err(|e| match e {
            Error::Sql(sqlx::Error::RowNotFound) => Status::not_found("World not found"),
            e => Status::internal(e.to_string()),
        })?);

        Ok(Response::new(WorldMetadataResponse { metadata }))
    }

    async fn subscribe_indexer(
        &self,
        request: Request<SubscribeIndexerRequest>,
    ) -> ServiceResult<Self::SubscribeIndexerStream> {
        let SubscribeIndexerRequest { contract_address } = request.into_inner();
        let rx = self
            .subscribe_indexer(Felt::from_bytes_be_slice(&contract_address))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeIndexerStream))
    }

    async fn subscribe_models(
        &self,
        request: Request<SubscribeModelsRequest>,
    ) -> ServiceResult<Self::SubscribeModelsStream> {
        let SubscribeModelsRequest { models_keys } = request.into_inner();
        let rx = self
            .subscribe_models(models_keys)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeModelsStream))
    }

    async fn subscribe_entities(
        &self,
        request: Request<SubscribeEntitiesRequest>,
    ) -> ServiceResult<Self::SubscribeEntitiesStream> {
        let SubscribeEntitiesRequest { clauses } = request.into_inner();
        let rx =
            self.subscribe_entities(clauses).await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }

    async fn update_entities_subscription(
        &self,
        request: Request<UpdateEntitiesSubscriptionRequest>,
    ) -> ServiceResult<()> {
        let UpdateEntitiesSubscriptionRequest { subscription_id, clauses } = request.into_inner();
        self.entity_manager
            .update_subscriber(
                subscription_id,
                clauses.into_iter().map(|keys| keys.into()).collect(),
            )
            .await;

        Ok(Response::new(()))
    }

    async fn retrieve_entities(
        &self,
        request: Request<RetrieveEntitiesRequest>,
    ) -> Result<Response<RetrieveEntitiesResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let entities = self
            .retrieve_entities(
                ENTITIES_TABLE,
                ENTITIES_MODEL_RELATION_TABLE,
                ENTITIES_ENTITY_RELATION_COLUMN,
                query,
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(entities))
    }

    async fn subscribe_event_messages(
        &self,
        request: Request<SubscribeEntitiesRequest>,
    ) -> ServiceResult<Self::SubscribeEntitiesStream> {
        let SubscribeEntitiesRequest { clauses } = request.into_inner();
        let rx = self
            .subscribe_event_messages(clauses)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }

    async fn update_event_messages_subscription(
        &self,
        request: Request<UpdateEntitiesSubscriptionRequest>,
    ) -> ServiceResult<()> {
        let UpdateEntitiesSubscriptionRequest { subscription_id, clauses } = request.into_inner();
        self.event_message_manager
            .update_subscriber(
                subscription_id,
                clauses.into_iter().map(|keys| keys.into()).collect(),
            )
            .await;

        Ok(Response::new(()))
    }

    async fn retrieve_event_messages(
        &self,
        request: Request<RetrieveEntitiesRequest>,
    ) -> Result<Response<RetrieveEntitiesResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let entities = self
            .retrieve_entities(
                EVENT_MESSAGES_TABLE,
                EVENT_MESSAGES_MODEL_RELATION_TABLE,
                EVENT_MESSAGES_ENTITY_RELATION_COLUMN,
                query,
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(entities))
    }

    async fn retrieve_events(
        &self,
        request: Request<RetrieveEventsRequest>,
    ) -> Result<Response<RetrieveEventsResponse>, Status> {
        let query = request
            .into_inner()
            .query
            .ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let events =
            self.retrieve_events(&query).await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(events))
    }

    async fn subscribe_events(
        &self,
        request: Request<proto::world::SubscribeEventsRequest>,
    ) -> ServiceResult<Self::SubscribeEventsStream> {
        let keys = request.into_inner().keys;
        let rx = self.subscribe_events(keys).await.map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEventsStream))
    }
}

pub async fn new(
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    pool: &Pool<Sqlite>,
    block_rx: Receiver<u64>,
    world_address: Felt,
    provider: Arc<JsonRpcClient<HttpTransport>>,
) -> Result<
    (SocketAddr, impl Future<Output = Result<(), tonic::transport::Error>> + 'static),
    std::io::Error,
> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::world::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let world = DojoWorld::new(pool.clone(), block_rx, world_address, provider);
    let server = WorldServer::new(world);

    let server_future = Server::builder()
        // GrpcWeb is over http1 so we must enable it.
        .accept_http1(true)
        .add_service(reflection)
        .add_service(tonic_web::enable(server))
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
            shutdown_rx.recv().await.map_or((), |_| ())
        });

    Ok((addr, server_future))
}
