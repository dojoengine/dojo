use std::collections::HashMap;

use dojo_types::schema::Ty;
use serde::{Deserialize, Serialize};
use starknet_core::utils::{cairo_short_string_to_felt, starknet_keccak};
use starknet_crypto::{pedersen_hash, poseidon_hash, poseidon_hash_many};
use starknet_ff::FieldElement;

use crate::errors::Error;

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

impl TokenAmount {
    pub fn encode(
        &self,
        name: &str,
        types: &HashMap<String, Vec<Field>>,
    ) -> Result<FieldElement, Error> {
        let mut hashes = Vec::new();

        let type_hash = encode_type(name, types);
        hashes.push(starknet_keccak(type_hash.as_bytes()));

        hashes.push(self.token_address);

        let amount_hash = self.amount.encode("amount", types)?;
        hashes.push(amount_hash);

        Ok(poseidon_hash_many(hashes.as_slice()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NftId {
    // ContractAddress
    pub collection_address: FieldElement,
    pub token_id: U256,
}

impl NftId {
    pub fn encode(
        &self,
        name: &str,
        types: &HashMap<String, Vec<Field>>,
    ) -> Result<FieldElement, Error> {
        let mut hashes = Vec::new();

        let type_hash = encode_type(name, types);
        hashes.push(starknet_keccak(type_hash.as_bytes()));

        hashes.push(self.collection_address);
        let token_id = self.token_id.encode("token_id", types)?;

        hashes.push(token_id);

        Ok(poseidon_hash_many(hashes.as_slice()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct U256 {
    pub low: u128,
    pub high: u128,
}

impl U256 {
    pub fn encode(
        &self,
        name: &str,
        types: &HashMap<String, Vec<Field>>,
    ) -> Result<FieldElement, Error> {
        let mut hashes = Vec::new();

        let type_hash = encode_type(name, types);
        hashes.push(starknet_keccak(type_hash.as_bytes()));

        hashes.push(FieldElement::from(self.low));
        hashes.push(FieldElement::from(self.high));

        Ok(poseidon_hash_many(hashes.as_slice()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Field {
    ParentType(ParentField),
    SimpleType(SimpleField),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PrimitiveType {
    Object(HashMap<String, PrimitiveType>),
    Array(Vec<PrimitiveType>),
    FieldElement(FieldElement),
    Bool(bool),
    // comprehensive representation of
    // String, ShortString and Selector
    String(String),
    USize(usize),
    U128(u128),
    I128(i128),
    TokenAmount(TokenAmount),
    NftId(NftId),
    U256(U256),
}

fn get_fields(name: &str, types: &HashMap<String, Vec<Field>>) -> Vec<Field> {
    println!("{:?}", name);
    match name {
        "TokenAmount" => vec![
            Field::SimpleType(SimpleField {
                name: "token_address".to_string(),
                r#type: "ContractAddress".to_string(),
            }),
            Field::SimpleType(SimpleField {
                name: "amount".to_string(),
                r#type: "u256".to_string(),
            }),
        ],
        "NftId" => vec![
            Field::SimpleType(SimpleField {
                name: "collection_address".to_string(),
                r#type: "ContractAddress".to_string(),
            }),
            Field::SimpleType(SimpleField {
                name: "token_id".to_string(),
                r#type: "u256".to_string(),
            }),
        ],
        "u256" => vec![
            Field::SimpleType(SimpleField {
                name: "low".to_string(),
                r#type: "u128".to_string(),
            }),
            Field::SimpleType(SimpleField {
                name: "high".to_string(),
                r#type: "u128".to_string(),
            }),
        ],
        _ => types[name].clone(),
    }
}

fn get_dependencies(
    name: &str,
    types: &HashMap<String, Vec<Field>>,
    dependencies: &mut Vec<String>,
) {
    if dependencies.contains(&name.to_string()) {
        return;
    }

    dependencies.push(name.to_string());

    for field in &get_fields(name, types) {
        let mut field_type = match field {
            Field::SimpleType(simple_field) => simple_field.r#type.clone(),
            Field::ParentType(parent_field) => parent_field.contains.clone(),
        };

        field_type = field_type.trim_end_matches("*").to_string();

        if types.contains_key(&field_type) && !dependencies.contains(&field_type) {
            get_dependencies(&field_type, types, dependencies);
        }
    }
}

pub fn encode_type(name: &str, types: &HashMap<String, Vec<Field>>) -> String {
    let mut type_hash = String::new();

    // get dependencies
    let mut dependencies: Vec<String> = Vec::new();
    get_dependencies(name, types, &mut dependencies);

    // sort dependencies
    dependencies.sort();

    for dep in dependencies {
        type_hash += &format!("\"{}\"", dep);

        type_hash += "(";

        let fields = get_fields(&dep, types);
        println!("{:?}", fields);
        for (idx, field) in fields.iter().enumerate() {
            match field {
                Field::SimpleType(simple_field) => {
                    // if ( at start and ) at end
                    if simple_field.r#type.starts_with('(') && simple_field.r#type.ends_with(')') {
                        let inner_types = &simple_field.r#type[1..simple_field.r#type.len() - 1]
                            .split(',')
                            .map(|t| if t != "" { format!("\"{}\"", t) } else { t.to_string() })
                            .collect::<Vec<String>>()
                            .join(",");
                        type_hash += &format!("\"{}\":({})", simple_field.name, inner_types);
                    } else {
                        type_hash +=
                            &format!("\"{}\":\"{}\"", simple_field.name, simple_field.r#type);
                    }
                }
                Field::ParentType(parent_field) => {
                    type_hash +=
                        &format!("\"{}\":\"{}\"", parent_field.name, parent_field.contains);
                }
            }

            if idx < fields.len() - 1 {
                type_hash += ",";
            }
        }

        type_hash += ")";
    }

    type_hash
}

impl PrimitiveType {
    fn get_value_type(
        &self,
        name: &str,
        types: &HashMap<String, Vec<Field>>,
    ) -> Result<String, Error> {
        for (key, value) in types {
            if key == name {
                return Ok(key.clone());
            }

            for field in value {
                match field {
                    Field::SimpleType(simple_field) => {
                        if simple_field.name == name {
                            return Ok(simple_field.r#type.clone());
                        }
                    }
                    Field::ParentType(parent_field) => {
                        if parent_field.name == name {
                            return Ok(parent_field.r#type.clone());
                        }
                    }
                }
            }
        }

        Err(Error::InvalidMessageError(format!("Field {} not found in types", name)))
    }

    pub fn encode(
        &self,
        name: &str,
        types: &HashMap<String, Vec<Field>>,
    ) -> Result<FieldElement, Error> {
        match self {
            PrimitiveType::Object(obj) => {
                let mut hashes = Vec::new();

                let type_hash = encode_type(name, types);
                hashes.push(starknet_keccak(type_hash.as_bytes()));

                for (field_name, value) in obj {
                    let field_hash = value.encode(field_name, types)?;
                    hashes.push(field_hash);
                }

                Ok(poseidon_hash_many(hashes.as_slice()))
            }
            PrimitiveType::Array(array) => Ok(poseidon_hash_many(
                array
                    .iter()
                    .map(|x| x.encode(name, types))
                    .collect::<Result<Vec<_>, _>>()?
                    .as_slice(),
            )),
            PrimitiveType::FieldElement(field_element) => Ok(*field_element),
            PrimitiveType::Bool(boolean) => {
                if *boolean {
                    Ok(FieldElement::from(1 as u32))
                } else {
                    Ok(FieldElement::from(0 as u32))
                }
            }
            PrimitiveType::String(string) => {
                // get value type
                let value_type = self.get_value_type(name, types)?;

                match value_type.as_str() {
                    "shortstring" => cairo_short_string_to_felt(string).map_err(|_| {
                        Error::InvalidMessageError("Invalid short string".to_string())
                    }),
                    "string" => {
                        let mut hashes = Vec::new();

                        let type_hash = encode_type(name, types);
                        hashes.push(starknet_keccak(type_hash.as_bytes()));

                        let string_hash = starknet_keccak(string.as_bytes());
                        hashes.push(string_hash);

                        Ok(poseidon_hash_many(hashes.as_slice()))
                    }
                    "selector" => Ok(starknet_keccak(string.as_bytes())),
                    _ => Err(Error::InvalidMessageError(format!(
                        "Invalid type {} for string",
                        value_type
                    ))),
                }
            }
            PrimitiveType::USize(usize) => Ok(FieldElement::from(*usize)),
            PrimitiveType::U128(u128) => Ok(FieldElement::from(*u128)),
            PrimitiveType::I128(i128) => Ok(FieldElement::from(*i128 as u128)),
            PrimitiveType::U256(u256) => u256.encode(name, types),
            PrimitiveType::NftId(nft_id) => nft_id.encode(name, types),
            PrimitiveType::TokenAmount(token_amount) => token_amount.encode(name, types),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub version: String,
    #[serde(rename = "chainId")]
    pub chain_id: String,
    pub revision: Option<String>,
}

impl Domain {
    pub fn encode(&self, types: &HashMap<String, Vec<Field>>) -> Result<FieldElement, Error> {
        let mut object = HashMap::new();

        object.insert("name".to_string(), PrimitiveType::String(self.name.clone()));
        object.insert("version".to_string(), PrimitiveType::String(self.version.clone()));
        object.insert("chain_id".to_string(), PrimitiveType::String(self.chain_id.clone()));
        if let Some(revision) = &self.revision {
            object.insert("revision".to_string(), PrimitiveType::String(revision.clone()));
        }

        PrimitiveType::Object(object).encode("StarknetDomain", types)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypedData {
    pub types: HashMap<String, Vec<Field>>,
    #[serde(rename = "primaryType")]
    pub primary_type: String,
    pub domain: Domain,
    pub message: HashMap<String, PrimitiveType>,
}

impl TypedData {
    pub fn encode(&self, account: FieldElement) -> Result<FieldElement, Error> {
        if self.domain.revision.clone().unwrap_or("1".to_string()) != "1" {
            return Err(Error::InvalidMessageError(
                "Legacy revision 0 is not supported".to_string(),
            ));
        }

        let prefix_message = starknet_keccak("StarkNet Message".as_bytes());

        // encode domain separator
        let domain_hash = self.domain.encode(&self.types)?;

        // encode message
        let message_hash =
            PrimitiveType::Object(self.message.clone()).encode(&self.primary_type, &self.types)?;

        // return full hash
        Ok(poseidon_hash_many(vec![prefix_message, domain_hash, account, message_hash].as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starknet_ff::FieldElement;

    // Helper function to create a FieldElement from a u64 for testing
    fn fe_from_u64(val: u64) -> FieldElement {
        FieldElement::from(val)
    }

    // Example test for TokenAmount encoding
    #[test]
    fn test_token_amount_encoding() {
        let token_address = fe_from_u64(123456789); // Example token address
        let amount  = U256 { low: 123, high: 456 }; // Example amount

        let token_amount = TokenAmount { token_address, amount };

        // Simulate types HashMap required for encoding
        let types = HashMap::new(); // This should be populated based on your actual types

        let encoded = token_amount.encode("TokenAmount", &types).unwrap();

        // Compare the result with expected value
        // This part is tricky without knowing the expected output, so you might want to assert
        // that encoded is not zero, or compare it against a known value if you have one.
        assert_ne!(encoded, FieldElement::ZERO);
    }

    // Example test for NftId encoding
    #[test]
    fn test_nft_id_encoding() {
        let collection_address = fe_from_u64(987654321); // Example collection address
        let token_id = U256 { low: 789, high: 654 }; // Example token id

        let nft_id = NftId { collection_address, token_id };

        let types = HashMap::new(); // Populate as needed

        let encoded = nft_id.encode("NftId", &types).unwrap();

        // Assert on the encoded result
        assert_ne!(encoded, FieldElement::ZERO);
    }

    #[test]
    fn test_read_json() {
        // deserialize from json file
        let path = "mocks/mail_StructArray.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        println!("{:?}", typed_data);

        let path = "mocks/example_enum.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        println!("{:?}", typed_data);

        let path = "mocks/example_presetTypes.json";
        let file = std::fs::File::open
        (path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        println!("{:?}", typed_data);
    }

    #[test]
    fn test_type_encode() {
        let path = "mocks/mail_StructArray.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types);

        assert_eq!(encoded, "\"Mail\"(\"from\":\"Person\",\"to\":\"Person\",\"posts_len\":\"felt\",\"posts\":\"Post*\")\"Person\"(\"name\":\"felt\",\"wallet\":\"felt\")\"Post\"(\"title\":\"felt\",\"content\":\"felt\")");

        let path = "mocks/example_enum.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types);

        assert_eq!(encoded, "\"Example\"(\"someEnum\":\"MyEnum\")\"MyEnum\"(\"Variant 1\":(),\"Variant 2\":(\"u128\",\"u128*\"),\"Variant 3\":(\"u128\"))");

        let path = "mocks/example_presetTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types);

        assert_eq!(encoded, "\"Example\"(\"n0\":\"TokenAmount\",\"n1\":\"NftId\")");
    }
}
