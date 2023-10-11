use async_graphql::dynamic::TypeRef;
use async_graphql::Name;
use dojo_types::primitive::Primitive;
use indexmap::IndexMap;
use lazy_static::lazy_static;

use crate::types::{GraphqlType, TypeData, TypeMapping};

lazy_static! {
    pub static ref ENTITY_TYPE_MAPPING: TypeMapping = IndexMap::from([
        (Name::new("id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
        (Name::new("keys"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (Name::new("model_names"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (Name::new("event_id"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (
            Name::new("created_at"),
            TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
        ),
        (
            Name::new("updated_at"),
            TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
        ),
    ]);
    pub static ref EVENT_TYPE_MAPPING: TypeMapping = IndexMap::from([
        (Name::new("id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
        (Name::new("keys"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (Name::new("data"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (
            Name::new("created_at"),
            TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
        ),
        (Name::new("transaction_hash"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
    ]);
    pub static ref MODEL_TYPE_MAPPING: TypeMapping = IndexMap::from([
        (Name::new("id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
        (Name::new("name"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (
            Name::new("class_hash"),
            TypeData::Simple(TypeRef::named(Primitive::Felt252(None).to_string())),
        ),
        (
            Name::new("transaction_hash"),
            TypeData::Simple(TypeRef::named(Primitive::Felt252(None).to_string())),
        ),
        (
            Name::new("created_at"),
            TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
        ),
    ]);
    pub static ref SYSTEM_CALL_TYPE_MAPPING: TypeMapping = IndexMap::from([
        (Name::new("id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
        (Name::new("transaction_hash"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (Name::new("data"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (Name::new("system_id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
        (
            Name::new("created_at"),
            TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
        ),
    ]);
    pub static ref SYSTEM_TYPE_MAPPING: TypeMapping = IndexMap::from([
        (Name::new("id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
        (Name::new("name"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
        (
            Name::new("class_hash"),
            TypeData::Simple(TypeRef::named(Primitive::Felt252(None).to_string())),
        ),
        (
            Name::new("transaction_hash"),
            TypeData::Simple(TypeRef::named(Primitive::Felt252(None).to_string())),
        ),
        (
            Name::new("created_at"),
            TypeData::Simple(TypeRef::named(GraphqlType::DateTime.to_string())),
        ),
    ]);
    pub static ref PAGE_INFO_TYPE_MAPPING: TypeMapping = TypeMapping::from([
        (Name::new("has_previous_page"), TypeData::Simple(TypeRef::named(TypeRef::BOOLEAN))),
        (Name::new("has_next_page"), TypeData::Simple(TypeRef::named(TypeRef::BOOLEAN))),
        (
            Name::new("start_cursor"),
            TypeData::Simple(TypeRef::named(GraphqlType::Cursor.to_string())),
        ),
        (
            Name::new("end_cursor"),
            TypeData::Simple(TypeRef::named(GraphqlType::Cursor.to_string())),
        ),
    ]);
    pub static ref METADATA_TYPE_MAPPING: TypeMapping = IndexMap::from([
        (Name::new("id"), TypeData::Simple(TypeRef::named(TypeRef::ID))),
        (Name::new("uri"), TypeData::Simple(TypeRef::named(TypeRef::STRING))),
    ]);
}
