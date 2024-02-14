use std::collections::HashMap;

use dojo_types::schema::Ty;
use starknet_ff::FieldElement;


pub struct Type {
    name: String,
    type_: String
}

pub struct Domain {
    name: String,
    version: String,
    chain_id: FieldElement
}

pub struct TypedData {
    types: HashMap<String, Type>,
    primary_type: String,
    domain: Domain,
    message: HashMap<String, Ty>
}