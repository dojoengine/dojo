use async_graphql::dynamic::{Enum, Field, FieldFuture, InputObject, Object, TypeRef};
use async_graphql::Value;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite};

use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use super::inputs::order_input::{order_argument, parse_order_argument, OrderInputObject};
use super::inputs::where_input::{parse_where_argument, where_argument, WhereInputObject};
use super::inputs::InputObjectTrait;
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::constants::{ENTITY_ID_COLUMN, ENTITY_TABLE, ID_COLUMN, INTERNAL_ENTITY_ID_KEY};
use crate::mapping::ENTITY_TYPE_MAPPING;
use crate::query::data::{count_rows, fetch_multiple_rows, fetch_single_row};
use crate::query::value_mapping_from_row;
use crate::types::TypeData;
use crate::utils::extract;

#[derive(FromRow, Deserialize, PartialEq, Eq)]
pub struct ModelMember {
    pub id: String,
    pub model_id: String,
    pub model_idx: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub type_enum: String,
    pub key: bool,
    pub created_at: DateTime<Utc>,
}

pub struct ModelDataObject {
    pub name: String,
    pub plural_name: String,
    pub type_name: String,
    pub type_mapping: TypeMapping,
    pub where_input: WhereInputObject,
    pub order_input: OrderInputObject,
}

impl ModelDataObject {
    pub fn new(name: String, type_name: String, type_mapping: TypeMapping) -> Self {
        let where_input = WhereInputObject::new(type_name.as_str(), &type_mapping);
        let order_input = OrderInputObject::new(type_name.as_str(), &type_mapping);
        let plural_name = format!("{}Models", name);
        Self { name, plural_name, type_name, type_mapping, where_input, order_input }
    }
}

impl ObjectTrait for ModelDataObject {
    fn name(&self) -> (&str, &str) {
        (&self.name, &self.plural_name)
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }

    fn input_objects(&self) -> Option<Vec<InputObject>> {
        Some(vec![self.where_input.input_object(), self.order_input.input_object()])
    }

    fn enum_objects(&self) -> Option<Vec<Enum>> {
        self.order_input.enum_objects()
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        let type_name = self.type_name.clone();
        let type_mapping = self.type_mapping.clone();
        let where_mapping = self.where_input.type_mapping.clone();
        let field_type = format!("{}Connection", self.type_name());

        let mut field = Field::new(self.name().1, TypeRef::named(field_type), move |ctx| {
            let type_mapping = type_mapping.clone();
            let where_mapping = where_mapping.clone();
            let type_name = type_name.clone();

            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let order = parse_order_argument(&ctx);
                let filters = parse_where_argument(&ctx, &where_mapping)?;
                let connection = parse_connection_arguments(&ctx)?;
                let id_column = "event_id";

                let total_count = count_rows(&mut conn, &type_name, &None, &filters).await?;
                let (data, page_info) = fetch_multiple_rows(
                    &mut conn,
                    &type_name,
                    id_column,
                    &None,
                    &order,
                    &filters,
                    &connection,
                    total_count,
                )
                .await?;
                let connection = connection_output(
                    &data,
                    &type_mapping,
                    &order,
                    id_column,
                    total_count,
                    true,
                    page_info,
                )?;

                Ok(Some(Value::Object(connection)))
            })
        });

        // Add relay connection fields (first, last, before, after, where)
        field = connection_arguments(field);
        field = where_argument(field, self.type_name());
        field = order_argument(field, self.type_name());

        Some(field)
    }

    fn objects(&self) -> Vec<Object> {
        let mut path_array = vec![self.type_name().to_string()];
        let mut objects =
            data_objects_recursion(self.type_name(), self.type_mapping(), &mut path_array);

        // root object requires entity_field association
        let mut root = objects.pop().unwrap();
        root = root.field(entity_field());

        objects.push(root);
        objects
    }
}

fn data_objects_recursion(
    type_name: &str,
    type_mapping: &TypeMapping,
    path_array: &mut Vec<String>,
) -> Vec<Object> {
    let mut objects = Vec::<Object>::new();

    for (_, type_data) in type_mapping {
        if let TypeData::Nested((nested_type, nested_mapping)) = type_data {
            path_array.push(nested_type.to_string());
            objects.extend(data_objects_recursion(
                &nested_type.to_string(),
                nested_mapping,
                &mut path_array.clone(),
            ));
        }
    }

    objects.push(object(type_name, type_mapping, path_array));
    objects
}

pub fn object(type_name: &str, type_mapping: &TypeMapping, path_array: &[String]) -> Object {
    let mut object = Object::new(type_name);

    for (field_name, type_data) in type_mapping.clone() {
        // For nested types, we need to remove prefix in path array
        let namespace = format!("{}_", path_array[0]);
        let table_name = path_array.join("$").replace(&namespace, "");
        let field = Field::new(field_name.to_string(), type_data.type_ref(), move |ctx| {
            let field_name = field_name.clone();
            let type_data = type_data.clone();
            let table_name = table_name.clone();

            return FieldFuture::new(async move {
                if let Some(value) = ctx.parent_value.as_value() {
                    // Nested types resolution
                    if let TypeData::Nested((_, nested_mapping)) = type_data {
                        return match ctx.parent_value.try_to_value()? {
                            Value::Object(indexmap) => {
                                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                                let entity_id =
                                    extract::<String>(indexmap, INTERNAL_ENTITY_ID_KEY)?;

                                // TODO: remove subqueries and use JOIN in parent query
                                let data = fetch_single_row(
                                    &mut conn,
                                    &table_name,
                                    ENTITY_ID_COLUMN,
                                    &entity_id,
                                )
                                .await?;
                                let result = value_mapping_from_row(&data, &nested_mapping, true)?;

                                Ok(Some(Value::Object(result)))
                            }
                            _ => Err("incorrect value, requires Value::Object".into()),
                        };
                    }

                    // Simple types resolution
                    return match value {
                        Value::Object(value_mapping) => {
                            Ok(Some(value_mapping.get(&field_name).unwrap().clone()))
                        }
                        _ => Err("Incorrect value, requires Value::Object".into()),
                    };
                }

                // Catch model union resolutions, async-graphql sends union types as IndexMap<Name,
                // ConstValue>
                if let Some(value_mapping) = ctx.parent_value.downcast_ref::<ValueMapping>() {
                    return Ok(Some(value_mapping.get(&field_name).unwrap().clone()));
                }

                Err("Field resolver only accepts Value or IndexMap".into())
            });
        });

        object = object.field(field);
    }

    object
}

fn entity_field() -> Field {
    Field::new("entity", TypeRef::named("World__Entity"), |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let entity_id = extract::<String>(indexmap, INTERNAL_ENTITY_ID_KEY)?;
                    let data =
                        fetch_single_row(&mut conn, ENTITY_TABLE, ID_COLUMN, &entity_id).await?;
                    let entity = value_mapping_from_row(&data, &ENTITY_TYPE_MAPPING, false)?;

                    Ok(Some(Value::Object(entity)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}
