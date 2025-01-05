use async_graphql::dynamic::FieldValue;
use async_graphql::{Name, Value};

use crate::constants::{ERC20_TOKEN_NAME, ERC20_TYPE_NAME, ERC721_TOKEN_NAME, ERC721_TYPE_NAME};
use crate::mapping::{ERC20_TOKEN_TYPE_MAPPING, ERC721_TOKEN_TYPE_MAPPING};
use crate::object::BasicObject;
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
