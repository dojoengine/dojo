use std::collections::HashSet;

use async_graphql::dynamic::{Object, Scalar, Schema, SchemaBuilder, SchemaError};
use lazy_static::lazy_static;
use sqlx::SqlitePool;

use super::component::{ComponentObject, COMPONENT_TYPE_MAPPING};
use super::entity::{EntityObject, ENTITY_TYPE_MAPPING};
use super::event::{EventObject, EVENT_TYPE_MAPPING};
use super::system::{SystemObject, SYSTEM_TYPE_MAPPING};
use super::system_call::{SystemCallObject, SYSTEM_CALL_TYPE_MAPPING};
use super::ObjectTrait;

lazy_static! {
    pub static ref SCALAR_TYPES: HashSet<&'static str> = HashSet::from([
        "U8",
        "U16",
        "U32",
        "U64",
        "U128",
        "U250",
        "U256",
        "Cursor",
        "Boolean",
        "Address",
        "DateTime",
        "FieldElement",
    ]);
    pub static ref NUMERIC_SCALAR_TYPES: HashSet<&'static str> =
        HashSet::from(["U8", "U16", "U32", "U64", "U128", "U250", "U256",]);
    pub static ref STRING_SCALAR_TYPES: HashSet<&'static str> =
        HashSet::from(["Cursor", "Address", "DateTime", "FieldElement",]);
}

pub async fn build_schema(pool: &SqlitePool) -> Result<Schema, SchemaError> {
    // // retrieve component schemas from database
    // let components: Vec<Component> = sqlx::query_as("SELECT * FROM components")
    //     .fetch_all(pool)
    //     .await
    //     .unwrap();

    let entity = EntityObject::new(ENTITY_TYPE_MAPPING.clone());
    let component = ComponentObject::new(COMPONENT_TYPE_MAPPING.clone());
    let system = SystemObject::new(SYSTEM_TYPE_MAPPING.clone());
    let event = EventObject::new(EVENT_TYPE_MAPPING.clone());
    let system_call = SystemCallObject::new(SYSTEM_CALL_TYPE_MAPPING.clone());

    let mut fields = entity.field_resolvers();
    fields.extend(component.field_resolvers());
    fields.extend(system.field_resolvers());
    fields.extend(event.field_resolvers());
    fields.extend(system_call.field_resolvers());

    let query_root =
        fields.into_iter().fold(Object::new("Query"), |query_root, field| query_root.field(field));

    // register custom scalars
    let mut schema_builder: SchemaBuilder = SCALAR_TYPES.iter().fold(
        Schema::build("Query", None, None),
        |schema_builder, scalar_type| {
            if *scalar_type == "Boolean" || *scalar_type == "ID" {
                schema_builder
            } else {
                schema_builder.register(Scalar::new(*scalar_type))
            }
        },
    );

    // register default gql objects
    schema_builder = schema_builder
        .register(entity.object())
        .register(component.object())
        .register(system.object())
        .register(event.object())
        .register(system_call.object())
        .register(query_root)
        .data(pool.clone());

    schema_builder.finish()
}
