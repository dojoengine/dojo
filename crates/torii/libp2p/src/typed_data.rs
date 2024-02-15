use std::collections::HashMap;

use crypto_bigint::U256;
use dojo_types::schema::Ty;
use starknet_core::utils::starknet_keccak;
use starknet_crypto::{pedersen_hash, poseidon_hash, poseidon_hash_many};
use starknet_ff::FieldElement;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleType {
    pub name: String,
    pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParentType {
    pub name: String,
    pub r#type: String,
    pub contains: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenAmount {
    // ContractAddress
    pub token_address: FieldElement,
    pub amount: U256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NftId {
    // ContractAddress
    pub collection_address: FieldElement,
    pub token_id: U256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Type {
    SimpleType(SimpleType),
    ParentType(ParentType),
}

impl Type {
    pub fn serialize(&self, types: HashMap<String, Vec<Type>>) -> String {
        match self {
            Type::SimpleType(simple_type) => {
                format!("\"{}\":\"{}\"", simple_type.name, simple_type.r#type)
            }
            Type::ParentType(parent_type) => {
                format!("\"{}\":\"{}\"", simple_type.name, types[parent_type.contains].r#type)
            }
        }
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PrimitiveType {
    Object(HashMap<String, PrimitiveType>),
    FieldElement(FieldElement),
    Bool(bool),
    String(String),
    Selector(String),
    U128(u128),
    I128(i128),
    ContractAddress(FieldElement),
    ClassHash(FieldElement),
    Timestamp(u128),
    U256(U256),
    // Maximum of 31 ascii characters
    ShortString(String),
    Enum(HashMap<String, Vec<PrimitiveType>>),
    NftId(NftId),
    TokenAmount(TokenAmount),
}

impl PrimitiveType {
    pub fn encode(&self, name: String, types: HashMap<String, Vec<Type>>) -> FieldElement {
        match self {
            PrimitiveType::Object(obj) => {

            }
            PrimitiveType::Array(array) => {
                poseidon_hash_many(array.iter().map(|x| x.encode()).collect())
            }
            PrimitiveType::FieldElement(field_element) => *field_element,
            PrimitiveType::Bool(boolean) => {
                if *boolean {
                    FieldElement::from(1)
                } else {
                    FieldElement::from(0)
                }
            }
            PrimitiveType::String(string) => poseidon_hash_many(
                string
                    .as_bytes()
                    .iter()
                    .map(|x| FieldElement::from(*x as u128))
                    .collect(),
            ),
            PrimitiveType::Selector(selector) => {
                starknet_keccak(selector.as_bytes())
            }
            PrimitiveType::U128(u128) => FieldElement::from(*u128),
            PrimitiveType::I128(i128) => FieldElement::from(*i128 as u128),
            PrimitiveType::ContractAddress(contract_address) => *contract_address,
            PrimitiveType::ClassHash(class_hash) => *class_hash,
            PrimitiveType::Timestamp(timestamp) => FieldElement::from(*timestamp),

        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub version: String,
    pub chain_id: FieldElement,
    pub revision: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypedData {
    pub types: HashMap<String, Vec<Type>>,
    pub primary_type: String,
    pub domain: Domain,
    pub message: HashMap<String, PrimitiveType>,
}

impl TypedData {
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        if self.domain.revision == 0 {
            return Err(Error::InvalidMessage("Invalid revision".to_string()));
        }

        let prefix_message = starknet_keccak("StarkNet Message".as_bytes());

        // encode domain separator
        types["StarknetDomain"]
    }
}
