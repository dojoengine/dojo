use async_graphql::dynamic::{Object, Scalar, Schema, SchemaError};
use sqlx::SqlitePool;

use super::component::ComponentObject;
use super::entity::EntityObject;
use super::event::EventObject;
use super::system::SystemObject;
use super::system_call::SystemCallObject;
use super::types::ScalarType;
use super::{ObjectTraitInstance, ObjectTraitStatic};

pub async fn build_schema(pool: &SqlitePool) -> Result<Schema, SchemaError> {
    // base gql objects
    let objects: Vec<Box<dyn ObjectTraitInstance>> = vec![
        Box::new(EntityObject::new()),
        Box::new(ComponentObject::new()),
        Box::new(SystemObject::new()),
        Box::new(EventObject::new()),
        Box::new(SystemCallObject::new()),
    ];

    // collect field resolvers
    let mut fields = Vec::new();
    for object in &objects {
        fields.extend(object.field_resolvers());
    }

    // add field resolvers to query root
    let mut query_root = Object::new("Query");
    for field in fields {
        query_root = query_root.field(field);
    }

    // register custom scalars
    let mut schema_builder = Schema::build("Query", None, None);
    for scalar_type in ScalarType::types().iter() {
        schema_builder = schema_builder.register(Scalar::new(*scalar_type));
    }

    // register base gql objects
    for object in &objects {
        schema_builder = schema_builder.register(object.create());
    }

    schema_builder.register(query_root).data(pool.clone()).finish()
}
