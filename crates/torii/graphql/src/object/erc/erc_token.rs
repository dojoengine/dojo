use async_graphql::dynamic::{
    Field, FieldValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Value};
use sqlx::{Pool, Row, Sqlite};
use tokio_stream::StreamExt;
use torii_sqlite::simple_broker::SimpleBroker;
use torii_sqlite::types::Token;

use crate::constants::{ERC20_TOKEN_NAME, ERC20_TYPE_NAME, ERC721_TOKEN_NAME, ERC721_TYPE_NAME};
use crate::mapping::{ERC20_TOKEN_TYPE_MAPPING, ERC721_TOKEN_TYPE_MAPPING, TOKEN_TYPE_MAPPING};
use crate::object::{BasicObject, ResolvableObject};
use crate::types::{TypeMapping, ValueMapping};

#[derive(Debug)]
pub struct Erc20TokenObject;

impl BasicObject for Erc20TokenObject {
    fn name(&self) -> (&str, &str) {
        ERC20_TOKEN_NAME
    }

    fn type_name(&self) -> &str {
        ERC20_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC20_TOKEN_TYPE_MAPPING
    }
}

#[derive(Debug)]
pub struct Erc721TokenObject;

impl BasicObject for Erc721TokenObject {
    fn name(&self) -> (&str, &str) {
        ERC721_TOKEN_NAME
    }

    fn type_name(&self) -> &str {
        ERC721_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC721_TOKEN_TYPE_MAPPING
    }
}

#[derive(Debug, Clone)]
pub enum ErcTokenType {
    Erc20(Erc20Token),
    Erc721(Erc721Token),
}

#[derive(Debug, Clone)]
pub struct Erc20Token {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub contract_address: String,
    pub amount: String,
}

#[derive(Debug, Clone)]
pub struct Erc721Token {
    pub name: String,
    pub symbol: String,
    pub token_id: String,
    pub contract_address: String,
    pub metadata: String,
    pub metadata_name: Option<String>,
    pub metadata_description: Option<String>,
    pub metadata_attributes: Option<String>,
    pub image_path: String,
}

impl ErcTokenType {
    pub fn to_field_value<'a>(self) -> FieldValue<'a> {
        match self {
            ErcTokenType::Erc20(token) => FieldValue::with_type(
                FieldValue::value(Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(token.name)),
                    (Name::new("symbol"), Value::String(token.symbol)),
                    (Name::new("decimals"), Value::from(token.decimals)),
                    (Name::new("contractAddress"), Value::String(token.contract_address)),
                    (Name::new("amount"), Value::String(token.amount)),
                ]))),
                ERC20_TYPE_NAME.to_string(),
            ),
            ErcTokenType::Erc721(token) => FieldValue::with_type(
                FieldValue::value(Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(token.name)),
                    (Name::new("symbol"), Value::String(token.symbol)),
                    (Name::new("tokenId"), Value::String(token.token_id)),
                    (Name::new("contractAddress"), Value::String(token.contract_address)),
                    (Name::new("metadata"), Value::String(token.metadata)),
                    (
                        Name::new("metadataName"),
                        token.metadata_name.map(Value::String).unwrap_or(Value::Null),
                    ),
                    (
                        Name::new("metadataDescription"),
                        token.metadata_description.map(Value::String).unwrap_or(Value::Null),
                    ),
                    (
                        Name::new("metadataAttributes"),
                        token.metadata_attributes.map(Value::String).unwrap_or(Value::Null),
                    ),
                    (Name::new("imagePath"), Value::String(token.image_path)),
                ]))),
                ERC721_TYPE_NAME.to_string(),
            ),
        }
    }
}

#[derive(Debug)]
pub struct TokenObject;

impl BasicObject for TokenObject {
    fn name(&self) -> (&str, &str) {
        ("tokens", "token")
    }

    fn type_name(&self) -> &str {
        "Token"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TOKEN_TYPE_MAPPING
    }
}

impl ResolvableObject for TokenObject {
    fn resolvers(&self) -> Vec<Field> {
        vec![]
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        Some(vec![SubscriptionField::new(
            "tokenUpdated",
            TypeRef::named_nn(self.type_name()),
            |ctx| {
                SubscriptionFieldFuture::new(async move {
                    let pool = ctx.data::<Pool<Sqlite>>()?;
                    Ok(SimpleBroker::<Token>::subscribe()
                        .then(move |token| {
                            let pool = pool.clone();
                            async move {
                                // Fetch complete token data including contract type
                                let query = "SELECT t.*, c.contract_type 
                                               FROM tokens t 
                                               JOIN contracts c ON t.contract_address = \
                                             c.contract_address 
                                               WHERE t.id = ?";

                                let row =
                                    match sqlx::query(query).bind(&token.id).fetch_one(&pool).await
                                    {
                                        Ok(row) => row,
                                        Err(_) => return None,
                                    };

                                let contract_type: String = row.get("contract_type");
                                let token_metadata = match contract_type.to_lowercase().as_str() {
                                    "erc20" => {
                                        let token = Erc20Token {
                                            contract_address: row.get("contract_address"),
                                            name: row.get("name"),
                                            symbol: row.get("symbol"),
                                            decimals: row.get("decimals"),
                                            amount: "0".to_string(), // New token has no balance
                                        };
                                        ErcTokenType::Erc20(token)
                                    }
                                    "erc721" => {
                                        let metadata_str: String = row.get("metadata");
                                        let (
                                            metadata_str,
                                            metadata_name,
                                            metadata_description,
                                            metadata_attributes,
                                            image_path,
                                        ) = if metadata_str.is_empty() {
                                            (String::new(), None, None, None, String::new())
                                        } else {
                                            let metadata: serde_json::Value =
                                                serde_json::from_str(&metadata_str)
                                                    .expect("metadata is always json");
                                            let metadata_name = metadata.get("name").map(|v| {
                                                v.to_string().trim_matches('"').to_string()
                                            });
                                            let metadata_description =
                                                metadata.get("description").map(|v| {
                                                    v.to_string().trim_matches('"').to_string()
                                                });
                                            let metadata_attributes =
                                                metadata.get("attributes").map(|v| {
                                                    v.to_string().trim_matches('"').to_string()
                                                });

                                            let contract_address: String =
                                                row.get("contract_address");
                                            let image_path = format!("{}/image", contract_address);

                                            (
                                                metadata_str,
                                                metadata_name,
                                                metadata_description,
                                                metadata_attributes,
                                                image_path,
                                            )
                                        };

                                        let token = Erc721Token {
                                            name: row.get("name"),
                                            metadata: metadata_str,
                                            contract_address: row.get("contract_address"),
                                            symbol: row.get("symbol"),
                                            token_id: "0".to_string(), /* New token has no
                                                                        * specific token_id */
                                            metadata_name,
                                            metadata_description,
                                            metadata_attributes,
                                            image_path,
                                        };
                                        ErcTokenType::Erc721(token)
                                    }
                                    _ => return None,
                                };

                                Some(Ok(FieldValue::value(Value::Object(ValueMapping::from([
                                    (Name::new("id"), Value::String(token.id)),
                                    (
                                        Name::new("contractAddress"),
                                        Value::String(token.contract_address),
                                    ),
                                    (Name::new("name"), Value::String(token.name)),
                                    (Name::new("symbol"), Value::String(token.symbol)),
                                    (Name::new("decimals"), Value::Number(token.decimals.into())),
                                    (
                                        Name::new("tokenMetadata"),
                                        token_metadata.to_field_value().as_value().unwrap().clone(),
                                    ),
                                ])))))
                            }
                        })
                        .filter_map(|result| result))
                })
            },
        )])
    }
}
