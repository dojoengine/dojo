use std::collections::HashSet;

use async_graphql::dynamic::{Object, Scalar, Schema, SchemaBuilder, SchemaError};
use lazy_static::lazy_static;
use sqlx::SqlitePool;

use super::component::ComponentObject;
use super::entity::EntityObject;
use super::event::EventObject;
use super::system::SystemObject;
use super::system_call::SystemCallObject;
use super::{ObjectTraitInstance, ObjectTraitStatic};

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

    // base gql objects
    let objects: Vec<Box<dyn ObjectTraitInstance>> = vec![
        Box::new(EntityObject::new()),
        Box::new(ComponentObject::new()),
        Box::new(SystemObject::new()),
        Box::new(EventObject::new()),
        Box::new(SystemCallObject::new()),
    ];

    let fields = objects.iter().fold(Vec::new(), |mut fields, object| {
        fields.extend(object.field_resolvers());
        fields
    });

    // add field resolvers to query root
    let query_root =
        fields.into_iter().fold(Object::new("Query"), |query_root, field| query_root.field(field));

    // register custom scalars
    let schema_builder: SchemaBuilder = SCALAR_TYPES.iter().fold(
        Schema::build("Query", None, None),
        |schema_builder, scalar_type| {
            if *scalar_type == "Boolean" || *scalar_type == "ID" {
                schema_builder
            } else {
                schema_builder.register(Scalar::new(*scalar_type))
            }
        },
    );

    // register base gql objects
    objects
        .iter()
        .fold(schema_builder, |schema_builder, object| schema_builder.register(object.create()))
        .register(query_root)
        .data(pool.clone())
        .finish()
}
