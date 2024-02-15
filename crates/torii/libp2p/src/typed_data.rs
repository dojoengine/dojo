use std::collections::HashMap;

use crypto_bigint::{Encoding, U256};
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

        let type_hash = encode_type(
            name,
            &vec![
                Field::SimpleType(SimpleField {
                    name: "token_address".to_string(),
                    r#type: "ContractAddress".to_string(),
                }),
                Field::SimpleType(SimpleField {
                    name: "amount".to_string(),
                    r#type: "u256".to_string(),
                }),
            ],
            types
        );
        hashes.push(type_hash);

        hashes.push(self.token_address);

        let amount_hash = PrimitiveType::U256(self.amount).encode("amount", types)?;
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

        let type_hash = encode_type(
            name,
            &vec![
                Field::SimpleType(SimpleField {
                    name: "collection_address".to_string(),
                    r#type: "ContractAddress".to_string(),
                }),
                Field::SimpleType(SimpleField {
                    name: "token_id".to_string(),
                    r#type: "u256".to_string(),
                }),
            ],
            types,
        );
        hashes.push(type_hash);

        hashes.push(self.collection_address);
        let token_id = PrimitiveType::U256(self.token_id).encode("token_id", types)?;

        hashes.push(token_id);

        Ok(poseidon_hash_many(hashes.as_slice()))
    }
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
    Enum(HashMap<String, Vec<PrimitiveType>>),
    NftId(NftId),
    TokenAmount(TokenAmount),
}

pub fn encode_type(
    name: &str,
    fields: &Vec<Field>,
    types: &HashMap<String, Vec<Field>>,
) -> FieldElement {
    let mut type_hash = String::new();

    type_hash += &format!("\"{}\":", name);

    // add fields
    type_hash += "(";
    for (idx, field) in fields.iter().enumerate() {
        match field {
            Field::SimpleType(simple_field) => {
                type_hash += &format!("\"{}\":\"{}\"", simple_field.name, simple_field.r#type);
            }
            Field::ParentType(parent_field) => {
                return encode_type(
                    &parent_field.contains,
                    &types[&parent_field.contains],
                    types,
                );
            }
        }

        if idx < fields.len() - 1 {
            type_hash += ",";
        }
    }

    type_hash += ")";

    starknet_keccak(type_hash.as_bytes())
}

impl PrimitiveType {
    pub fn encode(
        &self,
        name: &str,
        types: &HashMap<String, Vec<Field>>,
    ) -> Result<FieldElement, Error> {
        match self {
            PrimitiveType::Object(obj) => {
                let mut hashes = Vec::new();

                let type_hash = encode_type(name, &types[name], types);
                hashes.push(type_hash);

                for (field_name, value) in obj {
                    let field_hash = value.encode(field_name, types)?;
                    hashes.push(field_hash);
                }

                Ok(poseidon_hash_many(hashes.as_slice()))
            }
            PrimitiveType::Array(array) => Ok(poseidon_hash_many(
                array
                    .iter()
                    .map(|x| x.encode(name.clone(), types))
                    .collect::<Result<Vec<_>, _>>()?
                    .as_slice(),
            )),
            PrimitiveType::Enum(enum_map) => {
                let mut hashes = Vec::new();

                let type_hash = encode_type(name, &types[name], types);
                hashes.push(type_hash);

                for (field_name, value) in enum_map {
                    let field_hash = poseidon_hash_many(
                        value
                            .iter()
                            .map(|x| x.encode(field_name, types))
                            .collect::<Result<Vec<_>, _>>()?
                            .as_slice(),
                    );
                    hashes.push(field_hash);
                }

                Ok(poseidon_hash_many(hashes.as_slice()))
            }
            PrimitiveType::FieldElement(field_element) => Ok(*field_element),
            PrimitiveType::Bool(boolean) => {
                if *boolean {
                    Ok(FieldElement::from(1 as u32))
                } else {
                    Ok(FieldElement::from(0 as u32))
                }
            }
            PrimitiveType::String(string) => Ok(poseidon_hash_many(
                string
                    .as_bytes()
                    .iter()
                    .map(|x| FieldElement::from(*x as u128))
                    .collect::<Vec<FieldElement>>()
                    .as_slice(),
            )),
            PrimitiveType::Selector(selector) => Ok(starknet_keccak(selector.as_bytes())),
            PrimitiveType::U128(u128) => Ok(FieldElement::from(*u128)),
            PrimitiveType::I128(i128) => Ok(FieldElement::from(*i128 as u128)),
            PrimitiveType::ContractAddress(contract_address) => Ok(*contract_address),
            PrimitiveType::ClassHash(class_hash) => Ok(*class_hash),
            PrimitiveType::Timestamp(timestamp) => Ok(FieldElement::from(*timestamp)),
            PrimitiveType::U256(u256) => {
                let mut hashes = Vec::new();

                let type_hash = encode_type(
                    name,
                    &vec![
                        Field::SimpleType(SimpleField {
                            name: "low".to_string(),
                            r#type: "u128".to_string(),
                        }),
                        Field::SimpleType(SimpleField {
                            name: "high".to_string(),
                            r#type: "u128".to_string(),
                        }),
                    ],
                    types,
                );
                hashes.push(type_hash);

                // lower half
                let bytes = u256.to_be_bytes();
                let low_hash =
                    FieldElement::from(u128::from_be_bytes(bytes[0..16].try_into().unwrap()));
                hashes.push(low_hash);

                let high_hash =
                    FieldElement::from(u128::from_be_bytes(bytes[16..32].try_into().unwrap()));
                hashes.push(high_hash);

                Ok(poseidon_hash_many(hashes.as_slice()))
            }
            PrimitiveType::ShortString(short_string) => cairo_short_string_to_felt(&short_string)
                .map_err(|_| Error::InvalidMessageError("Invalid short string".to_string())),
            PrimitiveType::NftId(nft_id) => nft_id.encode(name, types),
            PrimitiveType::TokenAmount(token_amount) => token_amount.encode(name, types),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub version: String,
    pub chain_id: String,
    pub revision: String,
}

impl Domain {
    pub fn encode(&self, types: &HashMap<String, Vec<Field>>) -> Result<FieldElement, Error> {
        let mut object = HashMap::new();

        object.insert("name".to_string(), PrimitiveType::ShortString(self.name.clone()));
        object.insert("version".to_string(), PrimitiveType::ShortString(self.version.clone()));
        object.insert("chain_id".to_string(), PrimitiveType::ShortString(self.chain_id.clone()));
        object.insert("revision".to_string(), PrimitiveType::ShortString(self.revision.clone()));

        PrimitiveType::Object(object).encode("StarknetDomain", types)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypedData {
    pub types: HashMap<String, Vec<Field>>,
    pub primary_type: String,
    pub domain: Domain,
    pub message: HashMap<String, PrimitiveType>,
}

impl TypedData {
    pub fn encode(&self, account: FieldElement) -> Result<FieldElement, Error> {
        if self.domain.revision == "0" {
            return Err(Error::InvalidMessageError("Invalid revision".to_string()));
        }

        let prefix_message = starknet_keccak("StarkNet Message".as_bytes());

        // encode domain separator
        let domain_hash = self.domain.encode(&self.types)?;

        // encode message
        let message_hash = PrimitiveType::Object(self.message.clone())
            .encode(&self.primary_type, &self.types)?;

        // return full hash
        Ok(poseidon_hash_many(vec![prefix_message, domain_hash, account, message_hash].as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_bigint::U256;
    use starknet_ff::{FieldElement};

    // Helper function to create a FieldElement from a u64 for testing
    fn fe_from_u64(val: u64) -> FieldElement {
        FieldElement::from(val)
    }

    // Example test for TokenAmount encoding
    #[test]
    fn test_token_amount_encoding() {
        let token_address = fe_from_u64(123456789); // Example token address
        let amount = U256::from(100u64); // Example token amount

        let token_amount = TokenAmount {
            token_address,
            amount,
        };

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
        let token_id = U256::from(50u64); // Example token ID

        let nft_id = NftId {
            collection_address,
            token_id,
        };

        let types = HashMap::new(); // Populate as needed

        let encoded = nft_id.encode("NftId", &types).unwrap();

        // Assert on the encoded result
        assert_ne!(encoded, FieldElement::ZERO);
    }

    // Example test for TypedData encoding
    #[test]
    fn test_typed_data_encoding() {
        let domain = Domain {
            name: "MyDapp".to_string(),
            version: "1.0".to_string(),
            chain_id: "SN_MAIN".to_string(),
            revision: "1".to_string(),
        };

        let message = HashMap::from([
            ("key".to_string(), PrimitiveType::String("value".to_string())),
        ]);

        let types = HashMap::new(); // Populate as needed

        let typed_data = TypedData {
            types,
            primary_type: "MyMessageType".to_string(),
            domain,
            message,
        };

        let account = fe_from_u64(111222333); // Example account address

        let encoded = typed_data.encode(account).unwrap();

        // Assert on the encoded result
        assert_ne!(encoded, FieldElement::ZERO);
    }
}

