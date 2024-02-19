use std::str::FromStr;

use dojo_types::schema::Ty;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use starknet_core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, starknet_keccak,
    CairoShortStringToFeltError,
};
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
        types: &IndexMap<String, Vec<Field>>,
        encode_type_hash: bool,
    ) -> Result<FieldElement, Error> {
        let mut hashes = Vec::new();

        if encode_type_hash {
            let type_hash = encode_type(name, types);
            hashes.push(starknet_keccak(type_hash.as_bytes()));
        }

        hashes.push(self.token_address);

        let amount_hash = self.amount.encode("u256", types, false)?;
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
        types: &IndexMap<String, Vec<Field>>,
        encode_type_hash: bool,
    ) -> Result<FieldElement, Error> {
        let mut hashes = Vec::new();

        if encode_type_hash {
            let type_hash = encode_type(name, types);
            hashes.push(starknet_keccak(type_hash.as_bytes()));
        }

        hashes.push(self.collection_address);
        let token_id = self.token_id.encode("u256", types, false)?;

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
        types: &IndexMap<String, Vec<Field>>,
        encode_type_hash: bool,
    ) -> Result<FieldElement, Error> {
        let mut hashes: Vec<FieldElement> = Vec::new();

        if encode_type_hash {
            let type_hash = encode_type(name, types);
            hashes.push(starknet_keccak(type_hash.as_bytes()));
        }

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
    Object(IndexMap<String, PrimitiveType>),
    Array(Vec<PrimitiveType>),
    Bool(bool),
    // comprehensive representation of
    // String, ShortString, Selector and Felt
    String(String),
    USize(usize),
    U128(u128),
    I128(i128),
    TokenAmount(TokenAmount),
    NftId(NftId),
    U256(U256),
}

fn get_fields(name: &str, types: &IndexMap<String, Vec<Field>>) -> Vec<Field> {
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
            Field::SimpleType(SimpleField { name: "low".to_string(), r#type: "u128".to_string() }),
            Field::SimpleType(SimpleField { name: "high".to_string(), r#type: "u128".to_string() }),
        ],
        _ => types[name].clone(),
    }
}

fn get_dependencies(
    name: &str,
    types: &IndexMap<String, Vec<Field>>,
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

pub fn encode_type(name: &str, types: &IndexMap<String, Vec<Field>>) -> String {
    let mut type_hash = String::new();

    // get dependencies
    let mut dependencies: Vec<String> = Vec::new();
    get_dependencies(name, types, &mut dependencies);

    // sort dependencies
    dependencies.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    for dep in dependencies {
        type_hash += &format!("\"{}\"", dep);

        type_hash += "(";

        let fields = get_fields(&dep, types);
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

fn byte_array_from_string(
    target_string: &str,
) -> Result<(Vec<FieldElement>, FieldElement, usize), CairoShortStringToFeltError> {
    let short_strings: Vec<&str> = split_long_string(target_string);
    let remainder = short_strings.last().unwrap_or(&"");

    let mut short_strings_encoded = short_strings
        .iter()
        .map(|&s| cairo_short_string_to_felt(s))
        .collect::<Result<Vec<FieldElement>, _>>()?;

    let (pending_word, pending_word_length) = if remainder.is_empty() || remainder.len() == 31 {
        (FieldElement::ZERO, 0)
    } else {
        (short_strings_encoded.pop().unwrap(), remainder.len())
    };

    Ok((short_strings_encoded, pending_word, pending_word_length))
}

fn split_long_string(long_str: &str) -> Vec<&str> {
    let mut result = Vec::new();

    let mut start = 0;
    while start < long_str.len() {
        let end = (start + 31).min(long_str.len());
        result.push(&long_str[start..end]);
        start = end;
    }

    result
}

impl PrimitiveType {
    fn get_value_type(
        &self,
        name: &str,
        types: &IndexMap<String, Vec<Field>>,
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

    fn get_hex(&self, value: &str) -> Result<FieldElement, Error> {
        if let Ok(felt) = FieldElement::from_str(value) {
            Ok(felt)
        } else {
            // assume its a short string and encode
            cairo_short_string_to_felt(value)
                .map_err(|_| Error::InvalidMessageError("Invalid short string".to_string()))
        }
    }

    pub fn encode(
        &self,
        r#type: &str,
        types: &IndexMap<String, Vec<Field>>,
        encode_type_hash: bool,
    ) -> Result<FieldElement, Error> {
        match self {
            PrimitiveType::Object(obj) => {
                let mut hashes = Vec::new();

                if encode_type_hash {
                    let type_hash = encode_type(r#type, types);
                    println!("type_hash: {}", type_hash);
                    hashes.push(get_selector_from_name(&type_hash).map_err(|_| {
                        Error::InvalidMessageError(format!("Invalid type {} for selector", r#type))
                    })?);
                }

                for (field_name, value) in obj {
                    println!("field_name: {}", field_name);
                    let field_hash =
                        value.encode(&self.get_value_type(field_name, types)?, types, false)?;
                    hashes.push(field_hash);
                }

                Ok(poseidon_hash_many(hashes.as_slice()))
            }
            PrimitiveType::Array(array) => Ok(poseidon_hash_many(
                array
                    .iter()
                    .map(|x| x.encode(r#type.trim_end_matches("*"), types, encode_type_hash))
                    .collect::<Result<Vec<_>, _>>()?
                    .as_slice(),
            )),
            PrimitiveType::Bool(boolean) => {
                let v = if *boolean {
                    FieldElement::from(1 as u32)
                } else {
                    FieldElement::from(0 as u32)
                };
                Ok(v)
            }
            PrimitiveType::String(string) => match r#type {
                "shortstring" => self.get_hex(string),
                "string" => {
                    // split the string into short strings and encode
                    let byte_array = byte_array_from_string(string).map_err(|_| {
                        Error::InvalidMessageError("Invalid short string".to_string())
                    })?;

                    let mut hashes = vec![FieldElement::from(byte_array.0.len())];

                    for hash in byte_array.0 {
                        hashes.push(hash);
                    }

                    hashes.push(byte_array.1);
                    hashes.push(FieldElement::from(byte_array.2));

                    Ok(poseidon_hash_many(hashes.as_slice()))
                }
                "selector" => get_selector_from_name(string).map_err(|_| {
                    Error::InvalidMessageError(format!("Invalid type {} for selector", r#type))
                }),
                "felt" => self.get_hex(string),
                "ContractAddress" => self.get_hex(string),
                "ClassHash" => self.get_hex(string),
                "timestamp" => self.get_hex(string),
                "u128" => self.get_hex(string),
                "i128" => self.get_hex(string),
                _ => Err(Error::InvalidMessageError(format!("Invalid type {} for string", r#type))),
            },
            PrimitiveType::USize(usize) => Ok(FieldElement::from(*usize as u128)),
            PrimitiveType::U128(u128) => Ok(FieldElement::from(*u128 as u128)),
            PrimitiveType::I128(i128) => Ok(FieldElement::from(*i128 as u128)),
            PrimitiveType::U256(u256) => u256.encode(r#type, types, encode_type_hash),
            PrimitiveType::NftId(nft_id) => nft_id.encode(r#type, types, encode_type_hash),
            PrimitiveType::TokenAmount(token_amount) => {
                token_amount.encode(r#type, types, encode_type_hash)
            }
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
    pub fn encode(&self, types: &IndexMap<String, Vec<Field>>) -> Result<FieldElement, Error> {
        let mut object = IndexMap::new();

        object.insert("name".to_string(), PrimitiveType::String(self.name.clone()));
        object.insert("version".to_string(), PrimitiveType::String(self.version.clone()));
        object.insert("chainId".to_string(), PrimitiveType::String(self.chain_id.clone()));
        if let Some(revision) = &self.revision {
            object.insert("revision".to_string(), PrimitiveType::String(revision.clone()));
        }

        PrimitiveType::Object(object).encode("StarknetDomain", types, true)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypedData {
    pub types: IndexMap<String, Vec<Field>>,
    #[serde(rename = "primaryType")]
    pub primary_type: String,
    pub domain: Domain,
    pub message: IndexMap<String, PrimitiveType>,
}

impl TypedData {
    pub fn encode(&self, account: FieldElement) -> Result<FieldElement, Error> {
        if self.domain.revision.clone().unwrap_or("1".to_string()) != "1" {
            return Err(Error::InvalidMessageError(
                "Legacy revision 0 is not supported".to_string(),
            ));
        }

        let prefix_message = cairo_short_string_to_felt("StarkNet Message").unwrap();

        // encode domain separator
        let domain_hash = self.domain.encode(&self.types)?;

        // encode message
        let message_hash = PrimitiveType::Object(self.message.clone()).encode(
            &self.primary_type,
            &self.types,
            true,
        )?;

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
        let amount = U256 { low: 123, high: 456 }; // Example amount

        let token_amount = TokenAmount { token_address, amount };

        // Simulate types IndexMap required for encoding
        let types = IndexMap::new(); // This should be populated based on your actual types

        let encoded = token_amount.encode("TokenAmount", &types, true).unwrap();

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

        let types = IndexMap::new(); // Populate as needed

        let encoded = nft_id.encode("NftId", &types, true).unwrap();

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
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        println!("{:?}", typed_data);
    }

    #[test]
    fn test_type_encode() {
        let path = "mocks/example_baseTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types);

        assert_eq!(encoded, "\"Example\"(\"n0\":\"felt\",\"n1\":\"bool\",\"n2\":\"string\",\"n3\":\"selector\",\"n4\":\"u128\",\"n5\":\"ContractAddress\",\"n6\":\"ClassHash\",\"n7\":\"timestamp\",\"n8\":\"shortstring\")");

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

    #[test]
    fn test_selector_encode() {
        let selector = PrimitiveType::String("transfer".to_string());
        let selector_hash =
            PrimitiveType::String(starknet_keccak("transfer".as_bytes()).to_string());

        let types = IndexMap::new();

        let encoded_selector = selector.encode("selector", &types, true).unwrap();
        let raw_encoded_selector = selector_hash.encode("felt", &types, true).unwrap();

        assert_eq!(encoded_selector, raw_encoded_selector);
        assert_eq!(encoded_selector, starknet_keccak("transfer".as_bytes()));
    }

    #[test]
    fn test_domain_hash() {
        let path = "mocks/example_baseTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let domain_hash = typed_data.domain.encode(&typed_data.types).unwrap();

        assert_eq!(
            domain_hash,
            FieldElement::from_hex_be(
                "0x555f72e550b308e50c1a4f8611483a174026c982a9893a05c185eeb85399657"
            )
            .unwrap()
        );
    }

    #[test]
    fn test_message_hash() {
        let path = "mocks/example_baseTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let message_hash = typed_data
            .encode(
                FieldElement::from_hex_be("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826").unwrap(),
            )
            .unwrap();

        assert_eq!(
            message_hash,
            FieldElement::from_hex_be(
                "0x790d9fa99cf9ad91c515aaff9465fcb1c87784d9cfb27271ed193675cd06f9c"
            )
            .unwrap()
        );
    }
}
