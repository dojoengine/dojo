pub const DEFAULT_LIMIT: u64 = 10;
pub const BOOLEAN_TRUE: i64 = 1;

pub const ENTITY_TABLE: &str = "entities";
pub const EVENT_TABLE: &str = "events";
pub const MODEL_TABLE: &str = "models";
pub const TRANSACTION_TABLE: &str = "transactions";
pub const METADATA_TABLE: &str = "metadata";

pub const ID_COLUMN: &str = "id";
pub const EVENT_ID_COLUMN: &str = "event_id";
pub const ENTITY_ID_COLUMN: &str = "entity_id";
pub const JSON_COLUMN: &str = "json";

pub const INTERNAL_ENTITY_ID_KEY: &str = "$entity_id$";

// objects namespaced to avoid conflicts with user models
pub const ENTITY_TYPE_NAME: &str = "World__Entity";
pub const MODEL_TYPE_NAME: &str = "World__Model";
pub const EVENT_TYPE_NAME: &str = "World__Event";
pub const SOCIAL_TYPE_NAME: &str = "World__Social";
pub const CONTENT_TYPE_NAME: &str = "World__Content";
pub const METADATA_TYPE_NAME: &str = "World__Metadata";
pub const PAGE_INFO_TYPE_NAME: &str = "World__PageInfo";
pub const TRANSACTION_TYPE_NAME: &str = "World__Transaction";
pub const QUERY_TYPE_NAME: &str = "World__Query";
pub const SUBSCRIPTION_TYPE_NAME: &str = "World__Subscription";
pub const MODEL_ORDER_TYPE_NAME: &str = "World__ModelOrder";
pub const MODEL_ORDER_FIELD_TYPE_NAME: &str = "World__ModelOrderField";

// objects' single and plural names
pub const ENTITY_NAMES: (&str, &str) = ("entity", "entities");
pub const MODEL_NAMES: (&str, &str) = ("model", "models");
pub const EVENT_NAMES: (&str, &str) = ("event", "events");
pub const SOCIAL_NAMES: (&str, &str) = ("social", "socials");
pub const CONTENT_NAMES: (&str, &str) = ("content", "contents");
pub const METADATA_NAMES: (&str, &str) = ("metadata", "metadatas");
pub const TRANSACTION_NAMES: (&str, &str) = ("transaction", "transactions");

// misc
pub const ORDER_DIR_TYPE_NAME: &str = "OrderDirection";
pub const ORDER_ASC: &str = "ASC";
pub const ORDER_DESC: &str = "DESC";
