use core::slice::SlicePattern;
use std::collections::HashMap;

use crypto_bigint::U256;
use dojo_types::schema::Ty;
use starknet_core::utils::{cairo_short_string_to_felt, starknet_keccak};
use starknet_crypto::{pedersen_hash, poseidon_hash, poseidon_hash_many};
use starknet_ff::FieldElement;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleField {
    pub name: String,
    pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParentField {
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
pub enum Field {
    SimpleType(SimpleField),
    ParentType(ParentField),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PrimitiveType {
    Object(HashMap<String, PrimitiveType>),
    Array(Vec<PrimitiveType>),
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
    // Enum(HashMap<String, Vec<PrimitiveType>>),
    NftId(NftId),
    TokenAmount(TokenAmount),
}

impl PrimitiveType {
    fn encode_type(&self, name: String, fields: Vec<Field>, types: HashMap<String, Vec<Field>>) -> FieldElement {
        let mut type_hash = String::new();

        type_hash += &format!("\"{}\":", name);

        // add fields
        type_hash += "(";
        for field in types[name] {
            match field {
                Field::SimpleType(simple_field) => {
                    type_hash += &format!("\"{}\":\"{}\"", simple_field.name, simple_field.r#type);
                }
                Field::ParentType(parent_field) => {
                    // type_hash += &format!("\"{}\":\"{}\",", parent_field.name, parent_field.r#type);
                    // type_hash += &format!("\"{}\":\"{}\",", parent_field.name, parent_field.contains);
                }
            }

            if field != fields.last().unwrap() {
                type_hash += ",";
            }
        }

        type_hash += ")";

        starknet_keccak(type_hash.as_bytes())
    }

    pub fn encode(&self, name: String, types: HashMap<String, Vec<Field>>) -> FieldElement {
        match self {
            PrimitiveType::Object(obj) => {
                let mut hashes = Vec::new();
                
                let type_hash = self.encode_type(name, types[name], types);
                hashes.push(type_hash);

                for (field_name, value) in obj {
                    let field_hash = value.encode(field_name, types);
                    hashes.push(field_hash);
                }

                poseidon_hash_many(hashes.as_slice())
            }
            PrimitiveType::Array(array) => {
                poseidon_hash_many(array.iter().map(|x| x.encode(name.clone(), types)).collect())
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
            PrimitiveType::U256(u256) => {
                let mut hashes = Vec::new();
                
                let type_hash = self.encode_type(name, vec![
                    Field::SimpleType(SimpleField {
                        name: "low".to_string(),
                        r#type: "u128".to_string(),
                    }),
                    Field::SimpleType(SimpleField {
                        name: "high".to_string(),
                        r#type: "u128".to_string(),
                    }),
                ], types);
                hashes.push(type_hash);

                let low_hash = u256.low.encode("low".to_string(), types);
                hashes.push(low_hash);

                let high_hash = u256.high.encode("high".to_string(), types);
                hashes.push(high_hash);

                poseidon_hash_many(hashes.as_slice())
            }
            PrimitiveType::ShortString(short_string) => cairo_short_string_to_felt(&short_string),
            PrimitiveType::NftId(nft_id) => {
                let mut hashes = Vec::new();
                
                let type_hash = self.encode_type(name, vec![
                    Field::SimpleType(SimpleField {
                        name: "collection_address".to_string(),
                        r#type: "FieldElement".to_string(),
                    }),
                    Field::SimpleType(SimpleField {
                        name: "token_id".to_string(),
                        r#type: "U256".to_string(),
                    }),
                ], types);
                hashes.push(type_hash);

                let collection_address_hash = nft_id.collection_address.encode("collection_address".to_string(), types);
                hashes.push(collection_address_hash);

                let token_id_hash = nft_id.token_id.encode("token_id".to_string(), types);
                hashes.push(token_id_hash);

                poseidon_hash_many(hashes.as_slice())
            }
            PrimitiveType::TokenAmount(token_amount) => {
                let mut hashes = Vec::new();
                
                let type_hash = self.encode_type(name, vec![
                    Field::SimpleType(SimpleField {
                        name: "token_address".to_string(),
                        r#type: "FieldElement".to_string(),
                    }),
                    Field::SimpleType(SimpleField {
                        name: "amount".to_string(),
                        r#type: "U256".to_string(),
                    }),
                ], types);
                hashes.push(type_hash);

                let token_address_hash = token_amount.token_address.encode("token_address".to_string(), types);
                hashes.push(token_address_hash);

                let amount_hash = token_amount.amount.encode("amount".to_string(), types);
                hashes.push(amount_hash);

                poseidon_hash_many(hashes.as_slice())
            }
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
    pub types: HashMap<String, Vec<Field>>,
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
