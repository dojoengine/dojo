pub mod logger;
pub mod subscriptions;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use base64::prelude::BASE64_STANDARD_NO_PAD;
use base64::Engine;
use crypto_bigint::{Encoding, U256};
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
use subscriptions::token::TokenManager;
use subscriptions::token_balance::TokenBalanceManager;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::codec::CompressionEncoding;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tonic_web::GrpcWebLayer;
use torii_sqlite::cache::ModelCache;
use torii_sqlite::error::{Error, ParseError, QueryError};
use torii_sqlite::model::{fetch_entities, map_row_to_ty};
use torii_sqlite::types::{
    OrderBy, OrderDirection, Page, Pagination, PaginationDirection, Token, TokenBalance,
};
use torii_sqlite::utils::u256_to_sql_string;
use tower_http::cors::{AllowOrigin, CorsLayer};

use self::subscriptions::entity::EntityManager;
use self::subscriptions::event_message::EventMessageManager;
use self::subscriptions::model_diff::{ModelDiffRequest, StateDiffManager};
use crate::proto::types::clause::ClauseType;
use crate::proto::types::member_value::ValueType;
use crate::proto::types::LogicalOperator;
use crate::proto::world::world_server::WorldServer;
use crate::proto::world::{
    RetrieveControllersRequest, RetrieveControllersResponse, RetrieveEventMessagesRequest,
    RetrieveTokenBalancesRequest, RetrieveTokenBalancesResponse, RetrieveTokensRequest,
    RetrieveTokensResponse, SubscribeEntitiesRequest, SubscribeEntityResponse,
    SubscribeEventMessagesRequest, SubscribeEventsResponse, SubscribeIndexerRequest,
    SubscribeIndexerResponse, SubscribeTokenBalancesRequest, SubscribeTokenBalancesResponse,
    SubscribeTokensRequest, SubscribeTokensResponse, UpdateEventMessagesSubscriptionRequest,
    UpdateTokenBalancesSubscriptionRequest, UpdateTokenSubscriptionRequest, WorldMetadataRequest,
    WorldMetadataResponse,
};
use crate::proto::{self};
use crate::types::schema::SchemaError;
use crate::types::ComparisonOperator;

pub(crate) static ENTITIES_TABLE: &str = "entities";
pub(crate) static ENTITIES_MODEL_RELATION_TABLE: &str = "entity_model";
pub(crate) static ENTITIES_ENTITY_RELATION_COLUMN: &str = "internal_entity_id";

pub(crate) static ENTITIES_HISTORICAL_TABLE: &str = "entities_historical";

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
            SchemaError::FromUtf8(err) => ParseError::FromUtf8(err).into(),
        }
    }
}

impl From<Token> for proto::types::Token {
    fn from(value: Token) -> Self {
        Self {
            token_id: if value.token_id.is_empty() {
                U256::ZERO.to_be_bytes().to_vec()
            } else {
                U256::from_be_hex(value.token_id.trim_start_matches("0x")).to_be_bytes().to_vec()
            },
            contract_address: Felt::from_str(&value.contract_address)
                .unwrap()
                .to_bytes_be()
                .to_vec(),
            name: value.name,
            symbol: value.symbol,
            decimals: value.decimals as u32,
            metadata: value.metadata.as_bytes().to_vec(),
        }
    }
}

impl From<TokenBalance> for proto::types::TokenBalance {
    fn from(value: TokenBalance) -> Self {
        let id = value.token_id.split(':').collect::<Vec<&str>>();

        Self {
            balance: U256::from_be_hex(value.balance.trim_start_matches("0x"))
                .to_be_bytes()
                .to_vec(),
            account_address: Felt::from_str(&value.account_address).unwrap().to_bytes_be().to_vec(),
            contract_address: Felt::from_str(&value.contract_address)
                .unwrap()
                .to_bytes_be()
                .to_vec(),
            token_id: if id.len() == 2 {
                U256::from_be_hex(id[1].trim_start_matches("0x")).to_be_bytes().to_vec()
            } else {
                U256::ZERO.to_be_bytes().to_vec()
            },
        }
    }
}

impl From<proto::types::Pagination> for Pagination {
    fn from(value: proto::types::Pagination) -> Self {
        Pagination {
            cursor: if value.cursor.is_empty() { None } else { Some(value.cursor) },
            limit: if value.limit > 0 { Some(value.limit) } else { None },
            direction: match value.direction {
                0 => PaginationDirection::Forward,
                1 => PaginationDirection::Backward,
                _ => unreachable!(),
            },
            order_by: value.order_by.into_iter().map(|order_by| order_by.into()).collect(),
        }
    }
}

impl From<proto::types::OrderBy> for OrderBy {
    fn from(value: proto::types::OrderBy) -> Self {
        OrderBy {
            model: value.model,
            member: value.member,
            direction: match value.direction {
                0 => OrderDirection::Asc,
                1 => OrderDirection::Desc,
                _ => unreachable!(),
            },
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
    token_manager: Arc<TokenManager>,
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
        let token_manager = Arc::new(TokenManager::default());

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

        tokio::task::spawn(subscriptions::token::Service::new(Arc::clone(&token_manager)));

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
            token_manager,
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
        pagination: Pagination,
        no_hashed_keys: bool,
        models: Vec<String>,
    ) -> Result<Page<proto::types::Entity>, Error> {
        self.query_by_hashed_keys(
            table,
            model_relation_table,
            entity_relation_column,
            None,
            pagination,
            no_hashed_keys,
            models,
        )
        .await
    }

    async fn fetch_historical_entities(
        &self,
        table: &str,
        model_relation_table: &str,
        where_clause: &str,
        mut bind_values: Vec<String>,
        pagination: Pagination,
    ) -> Result<Page<proto::types::Entity>, Error> {
        if !pagination.order_by.is_empty() {
            return Err(Error::QueryError(QueryError::UnsupportedQuery(
                "Order by is not supported for historical entities".to_string(),
            )));
        }

        let mut conditions = Vec::new();
        if !where_clause.is_empty() {
            conditions.push(where_clause.to_string());
        }

        // Add cursor condition if present
        if let Some(ref cursor) = pagination.cursor {
            match pagination.direction {
                PaginationDirection::Forward => {
                    conditions.push(format!("{table}.event_id >= ?"));
                }
                PaginationDirection::Backward => {
                    conditions.push(format!("{table}.event_id <= ?"));
                }
            }
            bind_values.push(
                String::from_utf8(
                    BASE64_STANDARD_NO_PAD.decode(cursor).map_err(|_| Error::InvalidCursor)?,
                )
                .map_err(|_| Error::InvalidCursor)?,
            );
        }

        let where_clause = if !conditions.is_empty() {
            format!("WHERE {}", conditions.join(" AND "))
        } else {
            String::new()
        };

        let order_direction = match pagination.direction {
            PaginationDirection::Forward => "ASC",
            PaginationDirection::Backward => "DESC",
        };

        let query = format!(
            "SELECT {table}.id, {table}.data, {table}.model_id, {table}.event_id, \
             group_concat({model_relation_table}.model_id) as model_ids
            FROM {table}
            JOIN {model_relation_table} ON {table}.id = {model_relation_table}.entity_id
            {where_clause}
            GROUP BY {table}.event_id
            ORDER BY {table}.event_id {order_direction}
            LIMIT ?
            "
        );

        let mut query = sqlx::query_as(&query);
        for value in bind_values {
            query = query.bind(value);
        }
        query = query.bind(pagination.limit.unwrap_or(100) + 1);

        let db_entities: Vec<(String, String, String, String, String)> =
            query.fetch_all(&self.pool).await?;

        let mut entities = Vec::new();
        for (id, data, model_id, _, _) in &db_entities[..db_entities.len().saturating_sub(1)] {
            let hashed_keys =
                Felt::from_str(id).map_err(ParseError::FromStr)?.to_bytes_be().to_vec();
            let model = self
                .model_cache
                .model(&Felt::from_str(model_id).map_err(ParseError::FromStr)?)
                .await?;
            let mut schema = model.schema;
            schema.from_json_value(serde_json::from_str(data).map_err(ParseError::FromJsonStr)?)?;

            entities.push(proto::types::Entity {
                hashed_keys,
                models: vec![schema.as_struct().unwrap().clone().into()],
            });
        }

        // Get the next cursor from the last item's event_id if we fetched an extra one
        let next_cursor = if db_entities.len() > entities.len() {
            Some(db_entities.last().unwrap().3.clone()) // event_id is at index 3
        } else {
            None
        };

        Ok(Page {
            items: entities,
            next_cursor: next_cursor.map(|cursor| BASE64_STANDARD_NO_PAD.encode(cursor)),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_hashed_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        hashed_keys: Option<proto::types::HashedKeysClause>,
        pagination: Pagination,
        no_hashed_keys: bool,
        models: Vec<String>,
    ) -> Result<Page<proto::types::Entity>, Error> {
        let where_clause = match &hashed_keys {
            Some(hashed_keys) => {
                let ids =
                    hashed_keys.hashed_keys.iter().map(|_| "{table}.id = ?").collect::<Vec<_>>();
                ids.join(" OR ")
            }
            None => String::new(),
        };

        let bind_values = if let Some(hashed_keys) = hashed_keys {
            hashed_keys
                .hashed_keys
                .iter()
                .map(|key| format!("{:#x}", Felt::from_bytes_be_slice(key)))
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        let models =
            models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
        let schemas = self
            .model_cache
            .models(&models)
            .await?
            .iter()
            .map(|m| m.schema.clone())
            .collect::<Vec<_>>();

        let having_clause = models
            .iter()
            .map(|model| format!("INSTR(model_ids, '{:#x}') > 0", model))
            .collect::<Vec<_>>()
            .join(" OR ");

        if table.ends_with("_historical") {
            return self
                .fetch_historical_entities(
                    table,
                    model_relation_table,
                    &where_clause,
                    bind_values,
                    pagination,
                )
                .await;
        }

        let page = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            if !where_clause.is_empty() { Some(&where_clause) } else { None },
            if !having_clause.is_empty() { Some(&having_clause) } else { None },
            pagination,
            bind_values,
        )
        .await?;

        Ok(Page {
            items: page
                .items
                .par_iter()
                .map(|row| map_row_to_entity(row, &schemas, no_hashed_keys))
                .collect::<Result<Vec<_>, Error>>()?,
            next_cursor: page.next_cursor,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_keys(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        keys_clause: &proto::types::KeysClause,
        pagination: Pagination,
        no_hashed_keys: bool,
        models: Vec<String>,
    ) -> Result<Page<proto::types::Entity>, Error> {
        let keys_pattern = build_keys_pattern(keys_clause)?;
        let model_selectors: Vec<String> = keys_clause
            .models
            .iter()
            .map(|model| format!("{:#x}", compute_selector_from_tag(model)))
            .collect();

        let mut bind_values = vec![keys_pattern];
        let where_clause = if model_selectors.is_empty() {
            format!("{table}.keys REGEXP ?")
        } else {
            let model_selectors_len = model_selectors.len();
            bind_values.extend(model_selectors.clone());
            bind_values.extend(model_selectors);

            format!(
                "({table}.keys REGEXP ? AND {model_relation_table}.model_id IN ({})) OR \
                 {model_relation_table}.model_id NOT IN ({})",
                vec!["?"; model_selectors_len].join(", "),
                vec!["?"; model_selectors_len].join(", "),
            )
        };

        let models =
            models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
        let schemas = self
            .model_cache
            .models(&models)
            .await?
            .iter()
            .map(|m| m.schema.clone())
            .collect::<Vec<_>>();

        let having_clause = models
            .iter()
            .map(|model| format!("INSTR(model_ids, '{:#x}') > 0", model))
            .collect::<Vec<_>>()
            .join(" OR ");

        if table.ends_with("_historical") {
            return self
                .fetch_historical_entities(
                    table,
                    model_relation_table,
                    &where_clause,
                    bind_values,
                    pagination,
                )
                .await;
        }

        let page = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            Some(&where_clause),
            if !having_clause.is_empty() { Some(&having_clause) } else { None },
            pagination,
            bind_values,
        )
        .await?;

        Ok(Page {
            items: page
                .items
                .par_iter()
                .map(|row| map_row_to_entity(row, &schemas, no_hashed_keys))
                .collect::<Result<Vec<_>, Error>>()?,
            next_cursor: page.next_cursor,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_member(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        member_clause: proto::types::MemberClause,
        pagination: Pagination,
        no_hashed_keys: bool,
        models: Vec<String>,
    ) -> Result<Page<proto::types::Entity>, Error> {
        let models =
            models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
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
            return Ok(Page { items: Vec::new(), next_cursor: None });
        }

        let models_str = models_str.unwrap();

        let model_ids = models_str
            .split(',')
            .filter_map(|id| {
                let model_id = Felt::from_str(id).unwrap();
                if models.is_empty() || models.contains(&model_id) { Some(model_id) } else { None }
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
        let where_clause = format!(
            "[{}].[{}] {comparison_operator} {value}",
            member_clause.model, member_clause.member
        );

        let page = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            Some(&where_clause),
            None,
            pagination,
            bind_values,
        )
        .await?;

        Ok(Page {
            items: page
                .items
                .par_iter()
                .map(|row| map_row_to_entity(row, &schemas, no_hashed_keys))
                .collect::<Result<Vec<_>, Error>>()?,
            next_cursor: page.next_cursor,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn query_by_composite(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        composite: proto::types::CompositeClause,
        pagination: Pagination,
        no_hashed_keys: bool,
        models: Vec<String>,
    ) -> Result<Page<proto::types::Entity>, Error> {
        let (where_clause, bind_values) =
            build_composite_clause(table, model_relation_table, &composite)?;

        let models =
            models.iter().map(|model| compute_selector_from_tag(model)).collect::<Vec<_>>();
        let schemas = self
            .model_cache
            .models(&models)
            .await?
            .iter()
            .map(|m| m.schema.clone())
            .collect::<Vec<_>>();

        let having_clause = models
            .iter()
            .map(|model| format!("INSTR(model_ids, '{:#x}') > 0", model))
            .collect::<Vec<_>>()
            .join(" OR ");

        let page = fetch_entities(
            &self.pool,
            &schemas,
            table,
            model_relation_table,
            entity_relation_column,
            if where_clause.is_empty() { None } else { Some(&where_clause) },
            if having_clause.is_empty() { None } else { Some(&having_clause) },
            pagination,
            bind_values,
        )
        .await?;

        Ok(Page {
            items: page
                .items
                .par_iter()
                .map(|row| map_row_to_entity(row, &schemas, no_hashed_keys))
                .collect::<Result<Vec<_>, Error>>()?,
            next_cursor: page.next_cursor,
        })
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
        token_ids: Vec<U256>,
        limit: Option<u32>,
        cursor: Option<String>,
    ) -> Result<RetrieveTokensResponse, Error> {
        let mut query = "SELECT * FROM tokens".to_string();
        let mut bind_values = Vec::new();
        let mut conditions = Vec::new();

        if !contract_addresses.is_empty() {
            let placeholders = vec!["?"; contract_addresses.len()].join(", ");
            conditions.push(format!("contract_address IN ({})", placeholders));
            bind_values.extend(contract_addresses.iter().map(|addr| format!("{:#x}", addr)));
        }
        if !token_ids.is_empty() {
            let placeholders = vec!["?"; token_ids.len()].join(", ");
            conditions.push(format!("token_id IN ({})", placeholders));
            bind_values.extend(token_ids.iter().map(|id| u256_to_sql_string(&(*id).into())));
        }

        if let Some(cursor) = cursor {
            bind_values.push(
                String::from_utf8(
                    BASE64_STANDARD_NO_PAD.decode(cursor).map_err(|_| Error::InvalidCursor)?,
                )
                .map_err(|_| Error::InvalidCursor)?,
            );
            conditions.push("id >= ?".to_string());
        }

        if !conditions.is_empty() {
            query += &format!(" WHERE {}", conditions.join(" AND "));
        }

        query += " ORDER BY id LIMIT ?";
        bind_values.push((limit.unwrap_or(100) + 1).to_string());

        let mut query = sqlx::query_as(&query);
        for value in bind_values {
            query = query.bind(value);
        }

        let tokens: Vec<Token> = query.fetch_all(&self.pool).await?;
        let next_cursor = if tokens.len() > limit.unwrap_or(100) as usize {
            BASE64_STANDARD_NO_PAD.encode(tokens.pop().unwrap().id.to_string().as_bytes())
        } else {
            String::new()
        };

        let tokens = tokens.iter().map(|token| token.clone().into()).collect();
        Ok(RetrieveTokensResponse { tokens, next_cursor })
    }

    async fn retrieve_token_balances(
        &self,
        account_addresses: Vec<Felt>,
        contract_addresses: Vec<Felt>,
        token_ids: Vec<U256>,
        limit: Option<u32>,
        cursor: Option<String>,
    ) -> Result<RetrieveTokenBalancesResponse, Error> {
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

        if !token_ids.is_empty() {
            let placeholders = vec!["?"; token_ids.len()].join(", ");
            conditions
                .push(format!("SUBSTR(token_id, INSTR(token_id, ':') + 1) IN ({})", placeholders));
            bind_values.extend(token_ids.iter().map(|id| u256_to_sql_string(&(*id).into())));
        }

        if let Some(cursor) = cursor {
            bind_values.push(
                String::from_utf8(
                    BASE64_STANDARD_NO_PAD.decode(cursor).map_err(|_| Error::InvalidCursor)?,
                )
                .map_err(|_| Error::InvalidCursor)?,
            );
            conditions.push("id > ?".to_string());
        }

        if !conditions.is_empty() {
            query += &format!(" WHERE {}", conditions.join(" AND "));
        }

        query += " ORDER BY id LIMIT ?";
        bind_values.push((limit.unwrap_or(100) + 1).to_string());

        let mut query = sqlx::query_as(&query);
        for value in bind_values {
            query = query.bind(value);
        }

        let balances: Vec<TokenBalance> = query.fetch_all(&self.pool).await?;
        let next_cursor = if balances.len() > limit.unwrap_or(100) as usize {
            BASE64_STANDARD_NO_PAD.encode(balances.pop().unwrap().id.to_string().as_bytes())
        } else {
            String::new()
        };

        let balances = balances.iter().map(|balance| balance.clone().into()).collect();
        Ok(RetrieveTokenBalancesResponse { balances, next_cursor })
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

    async fn retrieve_entities(
        &self,
        table: &str,
        model_relation_table: &str,
        entity_relation_column: &str,
        query: proto::types::Query,
    ) -> Result<proto::world::RetrieveEntitiesResponse, Error> {
        let pagination = query.pagination.ok_or(QueryError::MissingParam("pagination".into()))?;
        let pagination: Pagination = pagination.into();

        let page = match query.clause {
            None => {
                self.entities_all(
                    table,
                    model_relation_table,
                    entity_relation_column,
                    pagination,
                    query.no_hashed_keys,
                    query.models,
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
                            pagination,
                            query.no_hashed_keys,
                            query.models,
                        )
                        .await?
                    }
                    ClauseType::Keys(keys) => {
                        self.query_by_keys(
                            table,
                            model_relation_table,
                            entity_relation_column,
                            &keys,
                            pagination,
                            query.no_hashed_keys,
                            query.models,
                        )
                        .await?
                    }
                    ClauseType::Member(member) => {
                        self.query_by_member(
                            table,
                            model_relation_table,
                            entity_relation_column,
                            member,
                            pagination,
                            query.no_hashed_keys,
                            query.models,
                        )
                        .await?
                    }
                    ClauseType::Composite(composite) => {
                        self.query_by_composite(
                            table,
                            model_relation_table,
                            entity_relation_column,
                            composite,
                            pagination,
                            query.no_hashed_keys,
                            query.models,
                        )
                        .await?
                    }
                }
            }
        };

        Ok(RetrieveEntitiesResponse {
            entities: page.items,
            next_cursor: page.next_cursor.unwrap_or_default(),
        })
    }

    async fn retrieve_events(
        &self,
        query: &proto::types::EventQuery,
    ) -> Result<proto::world::RetrieveEventsResponse, Error> {
        let limit = if query.limit > 0 { query.limit + 1 } else { 100 + 1 };

        let mut bind_values = Vec::new();
        let keys_pattern = if let Some(keys_clause) = &query.keys {
            build_keys_pattern(keys_clause)?
        } else {
            String::new()
        };

        let mut events_query = r#"
            SELECT id, keys, data, transaction_hash
            FROM events
        "#
        .to_string();

        if !keys_pattern.is_empty() {
            events_query = format!("{} WHERE keys REGEXP ?", events_query);
            bind_values.push(keys_pattern);
        }

        events_query = format!("{} ORDER BY id DESC LIMIT ?", events_query);
        bind_values.push(limit.to_string());

        if !query.cursor.is_empty() {
            events_query = format!("{} WHERE id >= ?", events_query);
            bind_values.push(
                String::from_utf8(
                    BASE64_STANDARD_NO_PAD
                        .decode(query.cursor)
                        .map_err(|_| Error::InvalidCursor)?,
                )
                .map_err(|_| Error::InvalidCursor)?,
            );
        }

        let mut row_events = sqlx::query_as(&events_query);
        for value in &bind_values {
            row_events = row_events.bind(value);
        }
        let mut row_events: Vec<(String, String, String, String)> =
            row_events.fetch_all(&self.pool).await?;

        let next_cursor = if row_events.len() > (limit - 1) as usize {
            BASE64_STANDARD_NO_PAD.encode(row_events.pop().unwrap().0.to_string().as_bytes())
        } else {
            String::new()
        };

        let events = row_events
            .iter()
            .map(|(_, keys, data, transaction_hash)| {
                map_row_to_event(&(keys, data, transaction_hash))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(RetrieveEventsResponse { events, next_cursor })
    }

    async fn retrieve_controllers(
        &self,
        contract_addresses: Vec<Felt>,
    ) -> Result<proto::world::RetrieveControllersResponse, Error> {
        let query = if contract_addresses.is_empty() {
            "SELECT address, username, deployed_at FROM controllers".to_string()
        } else {
            format!(
                "SELECT address, username, deployed_at FROM controllers WHERE address IN ({})",
                contract_addresses.iter().map(|_| "?".to_string()).collect::<Vec<_>>().join(", ")
            )
        };

        let mut db_query = sqlx::query_as::<_, (String, String, DateTime<Utc>)>(&query);
        for address in &contract_addresses {
            db_query = db_query.bind(format!("{:#x}", address));
        }

        let rows = db_query.fetch_all(&self.pool).await?;

        let controllers = rows
            .into_iter()
            .map(|(address, username, deployed_at)| proto::types::Controller {
                address: address.parse::<Felt>().unwrap().to_bytes_be().to_vec(),
                username,
                deployed_at_timestamp: deployed_at.timestamp() as u64,
            })
            .collect();

        Ok(RetrieveControllersResponse { controllers })
    }
}

fn process_event_field(data: &str) -> Result<Vec<Vec<u8>>, Error> {
    Ok(data
        .trim_end_matches('/')
        .split('/')
        .filter(|&d| !d.is_empty())
        .map(|d| Felt::from_str(d).map_err(ParseError::FromStr).map(|f| f.to_bytes_be().to_vec()))
        .collect::<Result<Vec<_>, _>>()?)
}

fn map_row_to_event(row: &(&str, &str, &str)) -> Result<proto::types::Event, Error> {
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
    model_relation_table: &str,
    composite: &proto::types::CompositeClause,
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
                let model_selectors: Vec<String> = keys
                    .models
                    .iter()
                    .map(|model| format!("{:#x}", compute_selector_from_tag(model)))
                    .collect();

                if model_selectors.is_empty() {
                    where_clauses.push(format!("({table}.keys REGEXP ?)"));
                } else {
                    // Add bind value placeholders for each model selector
                    let placeholders = vec!["?"; model_selectors.len()].join(", ");
                    where_clauses.push(format!(
                        "({table}.keys REGEXP ? AND {model_relation_table}.model_id IN ({})) OR \
                         {model_relation_table}.model_id NOT IN ({})",
                        placeholders, placeholders
                    ));
                    // Add each model selector twice (once for IN and once for NOT IN)
                    bind_values.extend(model_selectors.clone());
                    bind_values.extend(model_selectors);
                }
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
                    build_composite_clause(table, model_relation_table, nested)?;

                if !nested_where.is_empty() {
                    where_clauses.push(nested_where);
                }
                bind_values.extend(nested_values);
            }
        }
    }

    let where_clause = if !where_clauses.is_empty() {
        where_clauses.join(if is_or { " OR " } else { " AND " })
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
type SubscribeTokenBalancesResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeTokenBalancesResponse, Status>> + Send>>;
type SubscribeTokensResponseStream =
    Pin<Box<dyn Stream<Item = Result<SubscribeTokensResponse, Status>> + Send>>;

#[tonic::async_trait]
impl proto::world::world_server::World for DojoWorld {
    type SubscribeModelsStream = SubscribeModelsResponseStream;
    type SubscribeEntitiesStream = SubscribeEntitiesResponseStream;
    type SubscribeEventMessagesStream = SubscribeEntitiesResponseStream;
    type SubscribeEventsStream = SubscribeEventsResponseStream;
    type SubscribeIndexerStream = SubscribeIndexerResponseStream;
    type SubscribeTokenBalancesStream = SubscribeTokenBalancesResponseStream;
    type SubscribeTokensStream = SubscribeTokensResponseStream;

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

    async fn retrieve_entities(
        &self,
        request: Request<RetrieveEntitiesRequest>,
    ) -> Result<Response<RetrieveEntitiesResponse>, Status> {
        let RetrieveEntitiesRequest { query } = request.into_inner();
        let query = query.ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let entities = self
            .retrieve_entities(
                if query.historical { ENTITIES_HISTORICAL_TABLE } else { ENTITIES_TABLE },
                ENTITIES_MODEL_RELATION_TABLE,
                ENTITIES_ENTITY_RELATION_COLUMN,
                query,
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(entities))
    }

    async fn retrieve_event_messages(
        &self,
        request: Request<RetrieveEventMessagesRequest>,
    ) -> Result<Response<RetrieveEntitiesResponse>, Status> {
        let RetrieveEventMessagesRequest { query } = request.into_inner();
        let query = query.ok_or_else(|| Status::invalid_argument("Missing query argument"))?;

        let entities = self
            .retrieve_entities(
                if query.historical {
                    EVENT_MESSAGES_HISTORICAL_TABLE
                } else {
                    EVENT_MESSAGES_TABLE
                },
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

    async fn retrieve_controllers(
        &self,
        request: Request<RetrieveControllersRequest>,
    ) -> Result<Response<RetrieveControllersResponse>, Status> {
        let RetrieveControllersRequest { contract_addresses } = request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();

        let controllers = self
            .retrieve_controllers(contract_addresses)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(controllers))
    }

    async fn retrieve_tokens(
        &self,
        request: Request<RetrieveTokensRequest>,
    ) -> Result<Response<RetrieveTokensResponse>, Status> {
        let RetrieveTokensRequest { contract_addresses, token_ids, limit, cursor } =
            request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let token_ids =
            token_ids.iter().map(|id| crypto_bigint::U256::from_be_slice(id)).collect::<Vec<_>>();

        let tokens = self
            .retrieve_tokens(
                contract_addresses,
                token_ids,
                if limit > 0 { Some(limit) } else { None },
                if !cursor.is_empty() { Some(cursor) } else { None },
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(tokens))
    }

    async fn subscribe_tokens(
        &self,
        request: Request<SubscribeTokensRequest>,
    ) -> ServiceResult<Self::SubscribeTokensStream> {
        let SubscribeTokensRequest { contract_addresses, token_ids } = request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let token_ids = token_ids.iter().map(|id| U256::from_be_slice(id)).collect::<Vec<_>>();

        let rx = self
            .token_manager
            .add_subscriber(contract_addresses, token_ids)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeTokensStream))
    }

    async fn update_tokens_subscription(
        &self,
        request: Request<UpdateTokenSubscriptionRequest>,
    ) -> ServiceResult<()> {
        let UpdateTokenSubscriptionRequest { subscription_id, contract_addresses, token_ids } =
            request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let token_ids = token_ids.iter().map(|id| U256::from_be_slice(id)).collect::<Vec<_>>();

        self.token_manager.update_subscriber(subscription_id, contract_addresses, token_ids).await;
        Ok(Response::new(()))
    }

    async fn retrieve_token_balances(
        &self,
        request: Request<RetrieveTokenBalancesRequest>,
    ) -> Result<Response<RetrieveTokenBalancesResponse>, Status> {
        let RetrieveTokenBalancesRequest {
            account_addresses,
            contract_addresses,
            token_ids,
            limit,
            cursor,
        } = request.into_inner();
        let account_addresses = account_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let token_ids = token_ids.iter().map(|id| U256::from_be_slice(id)).collect::<Vec<_>>();

        let balances = self
            .retrieve_token_balances(
                account_addresses,
                contract_addresses,
                token_ids,
                if limit > 0 { Some(limit) } else { None },
                if !cursor.is_empty() { Some(cursor) } else { None },
            )
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
            .indexer_manager
            .add_subscriber(&self.pool, Felt::from_bytes_be_slice(&contract_address))
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
        let rx = self
            .entity_manager
            .add_subscriber(clauses.into_iter().map(|keys| keys.into()).collect())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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
        request: Request<SubscribeTokenBalancesRequest>,
    ) -> ServiceResult<Self::SubscribeTokenBalancesStream> {
        let SubscribeTokenBalancesRequest { contract_addresses, account_addresses, token_ids } =
            request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let account_addresses = account_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let token_ids = token_ids.iter().map(|id| U256::from_be_slice(id)).collect::<Vec<_>>();

        let rx = self
            .token_balance_manager
            .add_subscriber(contract_addresses, account_addresses, token_ids)
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
            token_ids,
        } = request.into_inner();
        let contract_addresses = contract_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let account_addresses = account_addresses
            .iter()
            .map(|address| Felt::from_bytes_be_slice(address))
            .collect::<Vec<_>>();
        let token_ids = token_ids.iter().map(|id| U256::from_be_slice(id)).collect::<Vec<_>>();

        self.token_balance_manager
            .update_subscriber(subscription_id, contract_addresses, account_addresses, token_ids)
            .await;
        Ok(Response::new(()))
    }

    async fn subscribe_event_messages(
        &self,
        request: Request<SubscribeEventMessagesRequest>,
    ) -> ServiceResult<Self::SubscribeEntitiesStream> {
        let SubscribeEventMessagesRequest { clauses } = request.into_inner();
        let rx = self
            .event_message_manager
            .add_subscriber(clauses.into_iter().map(|keys| keys.into()).collect())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(Box::pin(ReceiverStream::new(rx)) as Self::SubscribeEntitiesStream))
    }

    async fn update_event_messages_subscription(
        &self,
        request: Request<UpdateEventMessagesSubscriptionRequest>,
    ) -> ServiceResult<()> {
        let UpdateEventMessagesSubscriptionRequest { subscription_id, clauses } =
            request.into_inner();
        self.event_message_manager
            .update_subscriber(
                subscription_id,
                clauses.into_iter().map(|keys| keys.into()).collect(),
            )
            .await;

        Ok(Response::new(()))
    }

    async fn subscribe_events(
        &self,
        request: Request<proto::world::SubscribeEventsRequest>,
    ) -> ServiceResult<Self::SubscribeEventsStream> {
        let keys = request.into_inner().keys;
        let rx = self
            .event_manager
            .add_subscriber(keys.into_iter().map(|keys| keys.into()).collect())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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
