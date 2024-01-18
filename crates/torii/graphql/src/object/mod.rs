pub mod connection;
pub mod entity;
pub mod event;
pub mod inputs;
pub mod metadata;
pub mod model;
pub mod model_data;
pub mod transaction;

use async_graphql::dynamic::{
    Enum, Field, FieldFuture, InputObject, InputValue, Object, SubscriptionField, TypeRef,
};
use async_graphql::Value;
use sqlx::{Pool, Sqlite};

use self::connection::edge::EdgeObject;
use self::connection::{
    connection_arguments, connection_output, parse_connection_arguments, ConnectionObject,
};
use crate::constants::ID_COLUMN;
use crate::query::data::{count_rows, fetch_multiple_rows, fetch_single_row};
use crate::query::value_mapping_from_row;
use crate::types::{TypeMapping, ValueMapping};
use crate::utils::extract;

pub trait ObjectTrait: Send + Sync {
    // Name of the graphql object, singular and plural (eg "player" and "players")
    fn name(&self) -> (&str, &str);

    // Type name of the graphql object (eg "World__Player")
    fn type_name(&self) -> &str;

    // Type mapping defines the fields of the graphql object and their corresponding type
    fn type_mapping(&self) -> &TypeMapping;

    // Related field resolve to sibling graphql objects
    fn related_fields(&self) -> Option<Vec<Field>> {
        None
    }

    // Resolves subscriptions, returns current object (eg "PlayerAdded")
    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        None
    }

    // Input objects consist of {type_name}WhereInput for filtering and {type_name}Order for
    // ordering
    fn input_objects(&self) -> Option<Vec<InputObject>> {
        None
    }

    // Enum objects
    fn enum_objects(&self) -> Option<Vec<Enum>> {
        None
    }

    fn table_name(&self) -> Option<&str> {
        None
    }

    // Resolves single object queries, returns current object of type type_name (eg "Player")
    fn resolve_one(&self) -> Option<Field> {
        let type_mapping = self.type_mapping().clone();
        let table_name = self.table_name().unwrap().to_string();

        Some(
            Field::new(self.name().0, TypeRef::named_nn(self.type_name()), move |ctx| {
                let type_mapping = type_mapping.clone();
                let table_name = table_name.to_string();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let id = extract::<String>(ctx.args.as_index_map(), ID_COLUMN)?;
                    let data = fetch_single_row(&mut conn, &table_name, ID_COLUMN, &id).await?;
                    let model = value_mapping_from_row(&data, &type_mapping, false)?;
                    Ok(Some(Value::Object(model)))
                })
            })
            .argument(InputValue::new(ID_COLUMN, TypeRef::named_nn(TypeRef::ID))),
        )
    }

    // Resolves plural object queries, returns type of {type_name}Connection (eg "PlayerConnection")
    fn resolve_many(&self) -> Option<Field> {
        let type_mapping = self.type_mapping().clone();
        let table_name = self.table_name().unwrap().to_string();

        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            move |ctx| {
                let type_mapping = type_mapping.clone();
                let table_name = table_name.to_string();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let total_count = count_rows(&mut conn, &table_name, &None, &None).await?;
                    let (data, page_info) = fetch_multiple_rows(
                        &mut conn,
                        &table_name,
                        ID_COLUMN,
                        &None,
                        &None,
                        &None,
                        &connection,
                        total_count,
                    )
                    .await?;
                    let results = connection_output(
                        &data,
                        &type_mapping,
                        &None,
                        ID_COLUMN,
                        total_count,
                        false,
                        page_info,
                    )?;

                    Ok(Some(Value::Object(results)))
                })
            },
        );

        field = connection_arguments(field);

        Some(field)
    }

    // Connection type, if resolve_many is Some then register connection graphql obj, includes
    // {type_name}Connection and {type_name}Edge according to relay spec https://relay.dev/graphql/connections.htm
    fn connection(&self) -> Option<Vec<Object>> {
        self.resolve_many()?;

        let edge = EdgeObject::new(self.name().0.to_string(), self.type_name().to_string());
        let connection =
            ConnectionObject::new(self.name().0.to_string(), self.type_name().to_string());

        let mut objects = Vec::new();
        objects.extend(edge.objects());
        objects.extend(connection.objects());

        Some(objects)
    }

    fn objects(&self) -> Vec<Object> {
        let mut object = Object::new(self.type_name());

        for (field_name, type_data) in self.type_mapping().clone() {
            let field = Field::new(field_name.to_string(), type_data.type_ref(), move |ctx| {
                let field_name = field_name.clone();

                FieldFuture::new(async move {
                    match ctx.parent_value.try_to_value()? {
                        Value::Object(values) => {
                            Ok(Some(values.get(&field_name).unwrap().clone())) // safe unwrap
                        }
                        _ => Err("incorrect value, requires Value::Object".into()),
                    }
                })
            });

            object = object.field(field);
        }

        // Add related graphql objects (eg event, system)
        if let Some(fields) = self.related_fields() {
            for field in fields {
                object = object.field(field);
            }
        }
        vec![object]
    }
}
