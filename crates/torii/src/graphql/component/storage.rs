use std::collections::HashMap;

use async_graphql::dynamic::{Field, FieldFuture, FieldValue, Object, TypeRef};
use async_graphql::Value;
use sqlx::{Pool, Sqlite};

use super::{FieldTypeMapping, ObjectTraitInstance};

pub struct ComponentStorage {
    pub name: String,
    pub type_name: String,
    pub field_type_mappings: FieldTypeMapping,
}

impl ComponentStorage {
    pub fn from(name: String, type_name: String, field_type_mappings: FieldTypeMapping) -> Self {
        Self { name, type_name, field_type_mappings }
    }
}

impl ObjectTraitInstance for ComponentStorage {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn field_type_mappings(&self) -> &FieldTypeMapping {
        &self.field_type_mappings
    }

    fn object(&self) -> Object {
        self.field_type_mappings.iter().fold(
            Object::new(&self.type_name),
            |obj, (field_name, field_type)| {
                let inner_name = field_name.clone();

                obj.field(Field::new(field_name, TypeRef::named_nn(field_type), move |ctx| {
                    let field_name = inner_name.clone();

                    FieldFuture::new(async move {
                        let values =
                            ctx.parent_value.try_downcast_ref::<HashMap<String, Value>>()?;
                        Ok(Some(values.get(field_name.as_str()).expect("field not found").clone()))
                    })
                }))
            },
        )
    }

    fn field_resolvers(&self) -> Vec<Field> {
        vec![Field::new(self.name.as_str(), TypeRef::named_nn(self.type_name.as_str()), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;

                Ok(Some(FieldValue::owned_any(HashMap::from([(
                    "component".to_string(),
                    "Component".to_string(),
                )]))))
            })
        })]
    }
}
