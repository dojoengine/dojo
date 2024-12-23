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
use std::time::Duration;

use dojo_types::naming::compute_selector_from_tag;
use dojo_types::primitive::{Primitive, PrimitiveError};
use dojo_types::schema::Ty;
use dojo_world::contracts::naming::compute_selector_from_names;
use futures::Stream;
use http::HeaderName;
use proto::world::{
    RetrieveEntitiesRequest, RetrieveEntitiesResponse, RetrieveEventsRequest,
    RetrieveEventsResponse, SubscribeModelsRequest, SubscribeModelsResponse,
    UpdateEntitiesSubscriptionRequest,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlx::prelude::FromRow;
use sqlx::sqlite::SqliteRow;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{Pool, Row, Sqlite};
use starknet::core::types::Felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use subscriptions::event::EventManager;
use subscriptions::indexer::IndexerManager;
use subscriptions::token_balance::TokenBalanceManager;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{channel, Receiver};
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::codec::CompressionEncoding;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use torii_core::error::{Error, ParseError, QueryError};
use torii_core::model::{fetch_entities, map_row_to_ty};
use torii_core::sql::cache::ModelCache;
use torii_core::types::{Token, TokenBalance};
use tower_http::cors::{AllowOrigin, CorsLayer};

use self::subscriptions::entity::EntityManager;
use self::subscriptions::event_message::EventMessageManager;
use self::subscriptions::model_diff::{ModelDiffRequest, StateDiffManager};
use crate::proto::types::clause::ClauseType;
use crate::proto::types::member_value::ValueType;
use crate::proto::types::LogicalOperator;
use crate::proto::world::world_server::WorldServer;
use crate::proto::world::{
    RetrieveEntitiesStreamingResponse, RetrieveEventMessagesRequest, RetrieveTokenBalancesRequest,
    RetrieveTokenBalancesResponse, RetrieveTokensRequest, RetrieveTokensResponse,
    SubscribeEntitiesRequest, SubscribeEntityResponse, SubscribeEventMessagesRequest,
    SubscribeEventsResponse, SubscribeIndexerRequest, SubscribeIndexerResponse,
    SubscribeTokenBalancesResponse, UpdateEventMessagesSubscriptionRequest,
    UpdateTokenBalancesSubscriptionRequest, WorldMetadataRequest, WorldMetadataResponse,
};
use crate::proto::{self};
use crate::types::schema::SchemaError;
use crate::types::ComparisonOperator;

pub(crate) static ENTITIES_TABLE: &str = "entities";
pub(crate) static ENTITIES_MODEL_RELATION_TABLE: &str = "entity_model";
pub(crate) static ENTITIES_ENTITY_RELATION_COLUMN: &str = "internal_entity_id";

pub(crate) static EVENT_MESSAGES_TABLE: &str = "event_messages";
pub(crate) static EVENT_MESSAGES_MODEL_RELATION_TABLE: &str = "event_model";
pub(crate) static EVENT_MESSAGES_ENTITY_RELATION_COLUMN: &str = "internal_event_message_id";

pub(crate) static EVENT_MESSAGES_HISTORICAL_TABLE: &str = "event_messages_historical";

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

impl From<Token> for proto::types::Token {
    fn from(value: Token) -> Self {
        Self {
            contract_address: value.contract_address,
            name: value.name,
            symbol: value.symbol,
            decimals: value.decimals as u32,
            metadata: value.metadata,
        }
    }
}

impl From<TokenBalance> for proto::types::TokenBalance {
    fn from(value: TokenBalance) -> Self {
        Self {
            balance: value.balance,
            account_address: value.account_address,
            contract_address: value.contract_address,
            token_id: value.token_id,
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
    token_balance_manager: Arc<TokenBalanceManager>,
}

impl DojoWorld {
    pub fn new(
        pool: Pool<Sqlite>,
        block_rx: Receiver<u64>,
        world_address: Felt,
        provider: Arc<JsonRpcClient<HttpTransport>>,
        model_cache: Arc<ModelCache>,
    ) -> Self {
        let entity_manager = Arc::new(EntityManager::default());
        let event_message_manager = Arc::new(EventMessageManager::default());
        let event_manager = Arc::new(EventManager::default());
        let state_diff_manager = Arc::new(StateDiffManager::default());
        let indexer_manager = Arc::new(IndexerManager::default());
        let token_balance_manager = Arc::new(TokenBalanceManager::default());

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

        tokio::task::spawn(subscriptions::token_balance::Service::new(Arc::clone(
            &token_balance_manager,
        )));

        Self {
            pool,
            world_address,
            model_cache,
            entity_manager,
            event_message_manager,
            event_manager,
            state_diff_manager,
            indexer_manager,
            token_balance_manager,
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

    #[allow(clippy::too_many_arguments)]
    async fn entities_all(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        limit: u32,
        offset: u32,
        dont_include_hashed_keys: bool,
        order_by: Option<&str>,
        entity_models: Vec<String>,
        entity_updated_after: Option<String>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        self.query_by_hashed_keys(
            table,
            model_relation_table,
            entity_relation_column,
            None,
            Some(limit),
            Some(offset),
            dont_include_hashed_keys,
            order_by,
            entity_models,
            entity_updated_after,
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

    async fn fetch_historical_event_messages(
        &self,
        query: &str,
        bind_values: Vec<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<proto::types::Entity>, Error> {
        let mut query = sqlx::query_as(query);
        for value in bind_values {
            query = query.bind(value);
        }
        let db_entities: Vec<(String, String, String, String)> =
            query.bind(limit).bind(offset).fetch_all(&self.pool).await?;

        let mut entities = HashMap::new();
        for (id, data, model_id, _) in db_entities {
            let hashed_keys =
                Felt::from_str(&id).map_err(ParseError::FromStr)?.to_bytes_be().to_vec();
            let model = self
                .model_cache
                .model(&Felt::from_str(&model_id).map_err(ParseError::FromStr)?)
                .await?;
            let mut schema = model.schema;
            schema
                .from_json_value(serde_json::from_str(&data).map_err(ParseError::FromJsonStr)?)?;

            let entity = entities
                .entry(id)
                .or_insert_with(|| proto::types::Entity { hashed_keys, models: vec![] });
            entity.models.push(schema.as_struct().unwrap().clone().into());
        }

        Ok(entities.into_values().collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_hashed_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        hashed_keys: Option<proto::types::HashedKeysClause>,
        limit: Option<u32>,
        offset: Option<u32>,
        dont_include_hashed_keys: bool,
        order_by: Option<&str>,
        entity_models: Vec<String>,
        entity_updated_after: Option<String>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let where_clause = match &hashed_keys {
            Some(hashed_keys) => {
                let ids =
                    hashed_keys.hashed_keys.iter().map(|_| "{table}.id = ?").collect::<Vec<_>>();
                format!(
                    "{} {}",
                    ids.join(" OR "),
                    if entity_updated_after.is_some() {
                        format!("AND {table}.updated_at >= ?")
                    } else {
                        String::new()
                    }
                )
            }
            None => {
                if entity_updated_after.is_some() {
                    format!("{table}.updated_at >= ?")
                } else {
                    String::new()
                }
            }
        };

        let mut bind_values = vec![];
        if let Some(hashed_keys) = hashed_keys {
            bind_values = hashed_keys
                .hashed_keys
                .iter()
                .map(|key| format!("{:#x}", Felt::from_bytes_be_slice(key)))
                .collect::<Vec<_>>();
        }
        if let Some(entity_updated_after) = entity_updated_after.clone() {
            bind_values.push(entity_updated_after);
        }

        let entity_models =
            entity_models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
        let schemas = self
            .model_cache
            .models(&entity_models)
            .await?
            .iter()
            .map(|m| m.schema.clone())
            .collect::<Vec<_>>();

        let having_clause = entity_models
            .iter()
            .map(|model| format!("INSTR(model_ids, '{:#x}') > 0", model))
            .collect::<Vec<_>>()
            .join(" OR ");

        if table == EVENT_MESSAGES_HISTORICAL_TABLE {
            let count_query = format!(
                r#"
                SELECT COUNT(*) FROM {table}
                JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
                WHERE {where_clause}
                GROUP BY {table}.event_id
            "#
            );
            let mut total_count = sqlx::query_scalar(&count_query);
            for value in &bind_values {
                total_count = total_count.bind(value);
            }
            let total_count = total_count.fetch_one(&self.pool).await?;
            if total_count == 0 {
                return Ok((Vec::new(), 0));
            }

            let entities = self.fetch_historical_event_messages(
                &format!(
                    r#"
                SELECT {table}.id, {table}.data, {table}.model_id, group_concat({model_relation_table}.model_id) as model_ids
                FROM {table}
                JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
                WHERE {where_clause}
                GROUP BY {table}.event_id
                ORDER BY {table}.event_id DESC
             "#
                ),
                bind_values,
                limit,
                offset
            ).await?;
            return Ok((entities, total_count));
        }

        let (rows, total_count) = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            if !where_clause.is_empty() { Some(&where_clause) } else { None },
            if !having_clause.is_empty() { Some(&having_clause) } else { None },
            order_by,
            limit,
            offset,
            bind_values,
        )
        .await?;

        let entities = rows
            .par_iter()
            .map(|row| map_row_to_entity(row, &schemas, dont_include_hashed_keys))
            .collect::<Result<Vec<_>, Error>>()?;

        Ok((entities, total_count))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        keys_clause: &proto::types::KeysClause,
        limit: Option<u32>,
        offset: Option<u32>,
        dont_include_hashed_keys: bool,
        order_by: Option<&str>,
        entity_models: Vec<String>,
        entity_updated_after: Option<String>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let keys_pattern = build_keys_pattern(keys_clause)?;

        let where_clause = format!(
            "{table}.keys REGEXP ? {}",
            if entity_updated_after.is_some() {
                format!("AND {table}.updated_at >= ?")
            } else {
                String::new()
            }
        );

        let mut bind_values = vec![keys_pattern];
        if let Some(entity_updated_after) = entity_updated_after.clone() {
            bind_values.push(entity_updated_after);
        }

        let entity_models =
            entity_models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
        let schemas = self
            .model_cache
            .models(&entity_models)
            .await?
            .iter()
            .map(|m| m.schema.clone())
            .collect::<Vec<_>>();

        let having_clause = entity_models
            .iter()
            .map(|model| format!("INSTR(model_ids, '{:#x}') > 0", model))
            .collect::<Vec<_>>()
            .join(" OR ");

        if table == EVENT_MESSAGES_HISTORICAL_TABLE {
            let count_query = format!(
                r#"
                SELECT COUNT(*) FROM {table}
                JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
                WHERE {where_clause}
                GROUP BY {table}.event_id
            "#
            );
            let mut total_count = sqlx::query_scalar(&count_query);
            for value in &bind_values {
                total_count = total_count.bind(value);
            }
            let total_count = total_count.fetch_one(&self.pool).await?;
            if total_count == 0 {
                return Ok((Vec::new(), 0));
            }

            let entities = self.fetch_historical_event_messages(
                &format!(
                    r#"
                    SELECT {table}.id, {table}.data, {table}.model_id, group_concat({model_relation_table}.model_id) as model_ids
                    FROM {table}
                    JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
                    WHERE {where_clause}
                    GROUP BY {table}.event_id
                    ORDER BY {table}.event_id DESC
                 "#
                ),
                bind_values,
                limit,
                offset
            ).await?;
            return Ok((entities, total_count));
        }

        let (rows, total_count) = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            Some(&where_clause),
            if !having_clause.is_empty() { Some(&having_clause) } else { None },
            order_by,
            limit,
            offset,
            bind_values,
        )
        .await?;

        let entities = rows
            .par_iter()
            .map(|row| map_row_to_entity(row, &schemas, dont_include_hashed_keys))
            .collect::<Result<Vec<_>, Error>>()?;

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

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_member(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        member_clause: proto::types::MemberClause,
        limit: Option<u32>,
        offset: Option<u32>,
        dont_include_hashed_keys: bool,
        order_by: Option<&str>,
        entity_models: Vec<String>,
        entity_updated_after: Option<String>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let entity_models =
            entity_models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
        let comparison_operator = ComparisonOperator::from_repr(member_clause.operator as usize)
            .expect("invalid comparison operator");

        fn prepare_comparison(
            value: &proto::types::MemberValue,
            bind_values: &mut Vec<String>,
        ) -> Result<String, Error> {
            match &value.value_type {
                Some(ValueType::String(value)) => {
                    bind_values.push(value.to_string());
                    Ok("?".to_string())
                }
                Some(ValueType::Primitive(value)) => {
                    let primitive: Primitive = (value.clone()).try_into()?;
                    bind_values.push(primitive.to_sql_value());
                    Ok("?".to_string())
                }
                Some(ValueType::List(values)) => Ok(format!(
                    "({})",
                    values
                        .values
                        .iter()
                        .map(|v| prepare_comparison(v, bind_values))
                        .collect::<Result<Vec<String>, Error>>()?
                        .join(", ")
                )),
                None => Err(QueryError::MissingParam("value_type".into()).into()),
            }
        }

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
            .filter_map(|id| {
                let model_id = Felt::from_str(id).unwrap();
                if entity_models.is_empty() || entity_models.contains(&model_id) {
                    Some(model_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let schemas = self
            .model_cache
            .models(&model_ids)
            .await?
            .into_iter()
            .map(|m| m.schema)
            .collect::<Vec<_>>();

        // Use the member name directly as the column name since it's already flattened
        let mut bind_values = Vec::new();
        let value = prepare_comparison(
            &member_clause.value.clone().ok_or(QueryError::MissingParam("value".into()))?,
            &mut bind_values,
        )?;
        let mut where_clause = format!(
            "[{}].[{}] {comparison_operator} {value}",
            member_clause.model, member_clause.member
        );
        if entity_updated_after.is_some() {
            where_clause += &format!(" AND {table}.updated_at >= ?");
            bind_values.push(entity_updated_after.unwrap());
        }

        let (rows, total_count) = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            Some(&where_clause),
            None,
            order_by,
            limit,
            offset,
            bind_values,
        )
        .await?;

        let entities = rows
            .par_iter()
            .map(|row| map_row_to_entity(row, &schemas, dont_include_hashed_keys))
            .collect::<Result<Vec<_>, Error>>()?;

        Ok((entities, total_count))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_composite(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        composite: proto::types::CompositeClause,
        limit: Option<u32>,
        offset: Option<u32>,
        dont_include_hashed_keys: bool,
        order_by: Option<&str>,
        entity_models: Vec<String>,
        entity_updated_after: Option<String>,
    ) -> Result<(Vec<proto::types::Entity>, u32), Error> {
        let (where_clause, bind_values) =
            build_composite_clause(table, &composite, entity_updated_after)?;

        let entity_models =
            entity_models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
        let schemas = self
            .model_cache
            .models(&entity_models)
            .await?
            .into_iter()
            .map(|m| m.schema)
            .collect::<Vec<_>>();

        let having_clause = entity_models
            .iter()
            .map(|model| format!("INSTR(model_ids, '{:#x}') > 0", model))
            .collect::<Vec<_>>()
            .join(" OR ");

        let (rows, total_count) = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            if where_clause.is_empty() { None } else { Some(&where_clause) },
            if having_clause.is_empty() { None } else { Some(&having_clause) },
            order_by,
            limit,
            offset,
            bind_values,
        )
        .await?;

        let entities = rows
            .par_iter()
            .map(|row| map_row_to_entity(row, &schemas, dont_include_hashed_keys))
            .collect::<Result<Vec<_>, Error>>()?;

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

    async fn retrieve_tokens(
        &self,
        contract_addresses: Vec<Felt>,
    ) -> Result<RetrieveTokensResponse, Status> {
        let query = if contract_addresses.is_empty() {
            "SELECT * FROM tokens".to_string()
        } else {
            let placeholders = vec!["?"; contract_addresses.len()].join(", ");
            format!("SELECT * FROM tokens WHERE contract_address IN ({})", placeholders)
        };

        let mut query = sqlx::query_as(&query);
        for address in &contract_addresses {
            query = query.bind(format!("{:#x}", address));
        }

        let tokens: Vec<Token> =
            query.fetch_all(&self.pool).await.map_err(|e| Status::internal(e.to_string()))?;

        let tokens = tokens.iter().map(|token| token.clone().into()).collect();
        Ok(RetrieveTokensResponse { tokens })
    }

    async fn retrieve_token_balances(
        &self,
        account_addresses: Vec<Felt>,
        contract_addresses: Vec<Felt>,
    ) -> Result<RetrieveTokenBalancesResponse, Status> {
        let mut query = "SELECT * FROM token_balances".to_string();
        let mut bind_values = Vec::new();
        let mut conditions = Vec::new();

        if !account_addresses.is_empty() {
            let placeholders = vec!["?"; account_addresses.len()].join(", ");
            conditions.push(format!("account_address IN ({})", placeholders));
            bind_values.extend(account_addresses.iter().map(|addr| format!("{:#x}", addr)));
        }

        if !contract_addresses.is_empty() {
            let placeholders = vec!["?"; contract_addresses.len()].join(", ");
            conditions.push(format!("contract_address IN ({})", placeholders));
            bind_values.extend(contract_addresses.iter().map(|addr| format!("{:#x}", addr)));
        }

        if !conditions.is_empty() {
            query += &format!(" WHERE {}", conditions.join(" AND "));
        }

        let mut query = sqlx::query_as(&query);
        for value in bind_values {
            query = query.bind(value);
        }

        let balances: Vec<TokenBalance> =
            query.fetch_all(&self.pool).await.map_err(|e| Status::internal(e.to_string()))?;

        let balances = balances.iter().map(|balance| balance.clone().into()).collect();
        Ok(RetrieveTokenBalancesResponse { balances })
    }

    async fn subscribe_token_balances(
        &self,
        contract_addresses: Vec<Felt>,
        account_addresses: Vec<Felt>,
    ) -> Result<Receiver<Result<proto::world::SubscribeTokenBalancesResponse, tonic::Status>>, Error>
    {
        self.token_balance_manager.add_subscriber(contract_addresses, account_addresses).await
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
        let order_by = query
            .order_by
            .iter()
            .map(|order_by| {
                format!(
                    "[{}].[{}] {}",
                    order_by.model,
                    order_by.member,
                    match order_by.direction {
                        0 => "ASC",
                        1 => "DESC",
                        _ => unreachable!(),
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let order_by = if order_by.is_empty() { None } else { Some(order_by.as_str()) };

        let entity_updated_after = match query.entity_updated_after {
            0 => None,
            _ => Some(
                // This conversion would include a `UTC` suffix, which is not valid for the SQL
                // query when comparing the timestamp with equality.
                // To have `>=` working, we need to remove the `UTC` suffix.
                DateTime::<Utc>::from_timestamp(query.entity_updated_after as i64, 0)
                    .ok_or_else(|| {
                        Error::from(QueryError::InvalidTimestamp(query.entity_updated_after))
                    })?
                    .to_string()
                    .replace("UTC", "")
                    .trim()
                    .to_string(),
            ),
        };

        let (entities, total_count) = match query.clause {
            None => {
                self.entities_all(
                    table,
                    model_relation_table,
                    entity_relation_column,
                    query.limit,
                    query.offset,
                    query.dont_include_hashed_keys,
                    order_by,
                    query.entity_models,
                    entity_updated_after,
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
                            query.dont_include_hashed_keys,
                            order_by,
                            query.entity_models,
                            entity_updated_after,
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
                            query.dont_include_hashed_keys,
                            order_by,
                            query.entity_models,
                            entity_updated_after,
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
                            query.dont_include_hashed_keys,
                            order_by,
                            query.entity_models,
                            entity_updated_after,
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
                            query.dont_include_hashed_keys,
                            order_by,
                            query.entity_models,
                            entity_updated_after,
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
        historical: bool,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        self.event_message_manager
            .add_subscriber(clauses.into_iter().map(|keys| keys.into()).collect(), historical)
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
    schemas: &[Ty],
    dont_include_hashed_keys: bool,
) -> Result<proto::types::Entity, Error> {
    let hashed_keys = Felt::from_str(&row.get::<String, _>("id")).map_err(ParseError::FromStr)?;
    let model_ids = row
        .get::<String, _>("model_ids")
        .split(',')
        .map(|id| Felt::from_str(id).map_err(ParseError::FromStr))
        .collect::<Result<Vec<_>, _>>()?;

    let models = schemas
        .iter()
        .filter(|schema| model_ids.contains(&compute_selector_from_tag(&schema.name())))
        .map(|schema| {
            let mut ty = schema.clone();
            map_row_to_ty("", &schema.name(), &mut ty, row)?;
            Ok(ty.as_struct().unwrap().clone().into())
        })
        .collect::<Result<Vec<_>, Error>>()?;

    Ok(proto::types::Entity {
        hashed_keys: if !dont_include_hashed_keys {
            hashed_keys.to_bytes_be().to_vec()
        } else {
            vec![]
        },
        models,
    })
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
        keys_pattern += &format!("(/{})*", KEY_PATTERN);
    }
    keys_pattern += "/$";

    Ok(keys_pattern)
}

// builds a composite clause for a query
fn build_composite_clause(
    table: &str,
    composite: &proto::types::CompositeClause,
    entity_updated_after: Option<String>,
) -> Result<(String, Vec<String>), Error> {
    let is_or = composite.operator == LogicalOperator::Or as i32;
    let mut where_clauses = Vec::new();
    let mut bind_values = Vec::new();

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
                where_clauses.push(format!("({table}.id IN ({}))", ids));
            }
            ClauseType::Keys(keys) => {
                let keys_pattern = build_keys_pattern(keys)?;
                bind_values.push(keys_pattern);
                where_clauses.push(format!("({table}.keys REGEXP ?)"));

                // Add model checks for specified models

                // NOTE: disabled since we are now using the top level entity models

                // for model in &keys.models {
                //     let (namespace, model_name) = model
                //         .split_once('-')
                //         .ok_or(QueryError::InvalidNamespacedModel(model.clone()))?;
                //     let model_id = compute_selector_from_names(namespace, model_name);

                //     having_clauses.push(format!("INSTR(model_ids, '{:#x}') > 0", model_id));
                // }
            }
            ClauseType::Member(member) => {
                let comparison_operator = ComparisonOperator::from_repr(member.operator as usize)
                    .expect("invalid comparison operator");
                let value = member.value.clone().ok_or(QueryError::MissingParam("value".into()))?;
                fn prepare_comparison(
                    value: &proto::types::MemberValue,
                    bind_values: &mut Vec<String>,
                ) -> Result<String, Error> {
                    match &value.value_type {
                        Some(ValueType::String(value)) => {
                            bind_values.push(value.to_string());
                            Ok("?".to_string())
                        }
                        Some(ValueType::Primitive(value)) => {
                            let primitive: Primitive = (value.clone()).try_into()?;
                            bind_values.push(primitive.to_sql_value());
                            Ok("?".to_string())
                        }
                        Some(ValueType::List(values)) => Ok(format!(
                            "({})",
                            values
                                .values
                                .iter()
                                .map(|v| prepare_comparison(v, bind_values))
                                .collect::<Result<Vec<String>, Error>>()?
                                .join(", ")
                        )),
                        None => Err(QueryError::MissingParam("value_type".into()).into()),
                    }
                }
                let value = prepare_comparison(&value, &mut bind_values)?;

                let model = member.model.clone();

                // Use the column name directly since it's already flattened
                where_clauses
                    .push(format!("([{model}].[{}] {comparison_operator} {value})", member.member));
            }
            ClauseType::Composite(nested) => {
                // Handle nested composite by recursively building the clause
                let (nested_where, nested_values) =
                    build_composite_clause(table, nested, entity_updated_after.clone())?;

                if !nested_where.is_empty() {
                    where_clauses.push(nested_where);
                }
                bind_values.extend(nested_values);
            }
        }
    }

    let where_clause = if !where_clauses.is_empty() {
        format!(
            "{} {}",
            where_clauses.join(if is_or { " OR " } else { " AND " }),
            if let Some(entity_updated_after) = entity_updated_after.clone() {
                bind_values.push(entity_updated_after);
                format!("AND {table}.updated_at >= ?")
            } else {
                String::new()
            }
        )
    } else if let Some(entity_updated_after) = entity_updated_after.clone() {
        bind_values.push(entity_updated_after);
        format!("{table}.updated_at >= ?")
    } else {
        String::new()
    };

    Ok((where_clause, bind_values))
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
type RetrieveEntitiesStreamingResponseStream =
    Pin<Box<dyn Stream<Item = Result<RetrieveEntitiesStreamingResponse, Status>> + Send>>;
type SubscribeTokenBalancesResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeTokenBalancesResponse, Status>> + Send>>;

#[tonic::async_trait]
impl proto::world::world_server::World for DojoWorld {
    type SubscribeModelsStream = SubscribeModelsResponseStream;
    type SubscribeEntitiesStream = SubscribeEntitiesResponseStream;
    type SubscribeEventMessagesStream = SubscribeEntitiesResponseStream;
    type SubscribeEventsStream = SubscribeEventsResponseStream;
    type SubscribeIndexerStream = SubscribeIndexerResponseStream;
    type RetrieveEntitiesStreamingStream = RetrieveEntitiesStreamingResponseStream;
    type SubscribeTokenBalancesStream = SubscribeTokenBalancesResponseStream;

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

    async fn retrieve_tokens(
        &self,
        request: Request<RetrieveTokensRequest>,
    ) -> Result<Response<RetrieveTokensResponse>, Status> {
        let RetrieveTokensRequest { contract_addresses } = request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();

        let tokens = self
            .retrieve_tokens(contract_addresses)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(tokens))
    }

    async fn retrieve_token_balances(
        &self,
        request: Request<RetrieveTokenBalancesRequest>,
    ) -> Result<Response<RetrieveTokenBalancesResponse>, Status> {
        let RetrieveTokenBalancesRequest { account_addresses, contract_addresses } =
            request.into_inner();
        let account_addresses = account_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();

        let balances = self
            .retrieve_token_balances(account_addresses, contract_addresses)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(balances))
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

    async fn subscribe_token_balances(
        &self,
        request: Request<RetrieveTokenBalancesRequest>,
    ) -> ServiceResult<Self::SubscribeTokenBalancesStream> {
        let RetrieveTokenBalancesRequest { contract_addresses, account_addresses } =
            request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let account_addresses = account_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();

        let rx = self
            .subscribe_token_balances(contract_addresses, account_addresses)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeTokenBalancesStream))
    }

    async fn update_token_balances_subscription(
        &self,
        request: Request<UpdateTokenBalancesSubscriptionRequest>,
    ) -> ServiceResult<()> {
        let UpdateTokenBalancesSubscriptionRequest {
            subscription_id,
            contract_addresses,
            account_addresses,
        } = request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let account_addresses = account_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();

        self.token_balance_manager
            .update_subscriber(subscription_id, contract_addresses, account_addresses)
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

    async fn retrieve_entities_streaming(
        &self,
        request: Request<RetrieveEntitiesRequest>,
    ) -> ServiceResult<Self::RetrieveEntitiesStreamingStream> {
        let query = request
            .into_inner()
            .query
            .ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let (tx, rx) = channel(100);
        let res = self
            .retrieve_entities(
                ENTITIES_TABLE,
                ENTITIES_MODEL_RELATION_TABLE,
                ENTITIES_ENTITY_RELATION_COLUMN,
                query,
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        tokio::spawn(async move {
            for (i, entity) in res.entities.iter().enumerate() {
                tx.send(Ok(RetrieveEntitiesStreamingResponse {
                    entity: Some(entity.clone()),
                    remaining_count: (res.total_count - (i + 1) as u32),
                }))
                .await
                .unwrap();
            }
        });

        Ok(
            Response::new(
                Box::pin(ReceiverStream::new(rx)) as Self::RetrieveEntitiesStreamingStream
            ),
        )
    }

    async fn subscribe_event_messages(
        &self,
        request: Request<SubscribeEventMessagesRequest>,
    ) -> ServiceResult<Self::SubscribeEntitiesStream> {
        let SubscribeEventMessagesRequest { clauses, historical } = request.into_inner();
        let rx = self
            .subscribe_event_messages(clauses, historical)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }

    async fn update_event_messages_subscription(
        &self,
        request: Request<UpdateEventMessagesSubscriptionRequest>,
    ) -> ServiceResult<()> {
        let UpdateEventMessagesSubscriptionRequest { subscription_id, clauses, historical } =
            request.into_inner();
        self.event_message_manager
            .update_subscriber(
                subscription_id,
                clauses.into_iter().map(|keys| keys.into()).collect(),
                historical,
            )
            .await;

        Ok(Response::new(()))
    }

    async fn retrieve_event_messages(
        &self,
        request: Request<RetrieveEventMessagesRequest>,
    ) -> Result<Response<RetrieveEntitiesResponse>, Status> {
        let RetrieveEventMessagesRequest { query, historical } = request.into_inner();
        let query = query.ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let entities = self
            .retrieve_entities(
                if historical { EVENT_MESSAGES_HISTORICAL_TABLE } else { EVENT_MESSAGES_TABLE },
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

const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_EXPOSED_HEADERS: [&str; 4] =
    ["grpc-status", "grpc-message", "grpc-status-details-bin", "grpc-encoding"];
const DEFAULT_ALLOW_HEADERS: [&str; 6] = [
    "x-grpc-web",
    "content-type",
    "x-user-agent",
    "grpc-timeout",
    "grpc-accept-encoding",
    "grpc-encoding",
];

pub async fn new(
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    pool: &Pool<Sqlite>,
    block_rx: Receiver<u64>,
    world_address: Felt,
    provider: Arc<JsonRpcClient<HttpTransport>>,
    model_cache: Arc<ModelCache>,
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

    let world = DojoWorld::new(pool.clone(), block_rx, world_address, provider, model_cache);
    let server = WorldServer::new(world)
        .accept_compressed(CompressionEncoding::Gzip)
        .send_compressed(CompressionEncoding::Gzip);

    let server_future = Server::builder()
        // GrpcWeb is over http1 so we must enable it.
        .accept_http1(true)
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::mirror_request())
                .allow_credentials(true)
                .max_age(DEFAULT_MAX_AGE)
                .expose_headers(
                    DEFAULT_EXPOSED_HEADERS
                        .iter()
                        .cloned()
                        .map(HeaderName::from_static)
                        .collect::<Vec<HeaderName>>(),
                )
                .allow_headers(
                    DEFAULT_ALLOW_HEADERS
                        .iter()
                        .cloned()
                        .map(HeaderName::from_static)
                        .collect::<Vec<HeaderName>>(),
                ),
        )
        .layer(GrpcWebLayer::new())
        .add_service(reflection)
        .add_service(server)
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
            shutdown_rx.recv().await.map_or((), |_| ())
        });

    Ok((addr, server_future))
}
