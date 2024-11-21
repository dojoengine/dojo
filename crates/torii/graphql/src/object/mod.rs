pub mod connection;
pub mod entity;
pub mod erc;
pub mod event;
pub mod event_message;
pub mod inputs;
pub mod metadata;
pub mod model;
pub mod model_data;
pub mod transaction;

use async_graphql::dynamic::{
    Enum, Field, FieldFuture, FieldValue, InputObject, InputValue, Object, SubscriptionField,
    TypeRef,
};
use async_graphql::Value;
use convert_case::{Case, Casing};
use erc::erc_token::ErcTokenType;
use erc::token_transfer::TokenTransferNode;
use erc::{Connection, ConnectionEdge};
use sqlx::{Pool, Sqlite};

use self::connection::edge::EdgeObject;
use self::connection::{
    connection_arguments, connection_output, parse_connection_arguments, ConnectionObject,
};
use self::inputs::keys_input::parse_keys_argument;
use self::inputs::order_input::parse_order_argument;
use crate::query::data::{count_rows, fetch_multiple_rows, fetch_single_row};
use crate::query::value_mapping_from_row;
use crate::types::{TypeMapping, ValueMapping};
use crate::utils::extract;

#[allow(missing_debug_implementations)]
pub enum ObjectVariant {
    Basic(Box<dyn BasicObject>),
    Resolvable(Box<dyn ResolvableObject>),
}

pub trait BasicObject: Send + Sync {
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

    // Graphql objects that are created from the type mapping
    fn objects(&self) -> Vec<Object> {
        let mut object = Object::new(self.type_name());

        for (field_name, type_data) in self.type_mapping().clone() {
            let field = Field::new(field_name.to_string(), type_data.type_ref(), move |ctx| {
                let field_name = field_name.clone();

                FieldFuture::new(async move {
                    match ctx.parent_value.try_to_value() {
                        Ok(Value::Object(values)) => {
                            // safe unwrap
                            return Ok(Some(FieldValue::value(
                                values.get(&field_name).unwrap().clone(),
                            )));
                        }
                        // if the parent is `Value` then it must be a Object
                        Ok(_) => return Err("incorrect value, requires Value::Object".into()),
                        _ => {}
                    };

                    // if its not we try to downcast to known types which is a special case for
                    // tokenBalances and tokenTransfers queries

                    if let Ok(values) =
                        ctx.parent_value.try_downcast_ref::<Connection<ErcTokenType>>()
                    {
                        match field_name.as_str() {
                            "edges" => {
                                return Ok(Some(FieldValue::list(
                                    values
                                        .edges
                                        .iter()
                                        .map(FieldValue::borrowed_any)
                                        .collect::<Vec<FieldValue<'_>>>(),
                                )));
                            }
                            "pageInfo" => {
                                return Ok(Some(FieldValue::value(values.page_info.clone())));
                            }
                            "totalCount" => {
                                return Ok(Some(FieldValue::value(Value::from(
                                    values.total_count,
                                ))));
                            }
                            _ => return Err("incorrect value, requires Value::Object".into()),
                        }
                    }

                    if let Ok(values) =
                        ctx.parent_value.try_downcast_ref::<ConnectionEdge<ErcTokenType>>()
                    {
                        match field_name.as_str() {
                            "node" => return Ok(Some(FieldValue::borrowed_any(&values.node))),
                            "cursor" => {
                                return Ok(Some(FieldValue::value(Value::String(
                                    values.cursor.clone(),
                                ))));
                            }
                            _ => return Err("incorrect value, requires Value::Object".into()),
                        }
                    }

                    if let Ok(values) =
                        ctx.parent_value.try_downcast_ref::<Connection<TokenTransferNode>>()
                    {
                        match field_name.as_str() {
                            "edges" => {
                                return Ok(Some(FieldValue::list(
                                    values
                                        .edges
                                        .iter()
                                        .map(FieldValue::borrowed_any)
                                        .collect::<Vec<FieldValue<'_>>>(),
                                )));
                            }
                            "pageInfo" => {
                                return Ok(Some(FieldValue::value(values.page_info.clone())));
                            }
                            "totalCount" => {
                                return Ok(Some(FieldValue::value(Value::from(
                                    values.total_count,
                                ))));
                            }
                            _ => return Err("incorrect value, requires Value::Object".into()),
                        }
                    }

                    if let Ok(values) =
                        ctx.parent_value.try_downcast_ref::<ConnectionEdge<TokenTransferNode>>()
                    {
                        match field_name.as_str() {
                            "node" => return Ok(Some(FieldValue::borrowed_any(&values.node))),
                            "cursor" => {
                                return Ok(Some(FieldValue::value(Value::String(
                                    values.cursor.clone(),
                                ))));
                            }
                            _ => return Err("incorrect value, requires Value::Object".into()),
                        }
                    }

                    if let Ok(values) = ctx.parent_value.try_downcast_ref::<TokenTransferNode>() {
                        match field_name.as_str() {
                            "from" => {
                                return Ok(Some(FieldValue::value(Value::String(
                                    values.from.clone(),
                                ))));
                            }
                            "to" => {
                                return Ok(Some(FieldValue::value(Value::String(
                                    values.to.clone(),
                                ))));
                            }
                            "executedAt" => {
                                return Ok(Some(FieldValue::value(Value::String(
                                    values.executed_at.clone(),
                                ))));
                            }
                            "tokenMetadata" => {
                                return Ok(Some(values.clone().token_metadata.to_field_value()));
                            }
                            "transactionHash" => {
                                return Ok(Some(FieldValue::value(Value::String(
                                    values.transaction_hash.clone(),
                                ))));
                            }
                            _ => return Err("incorrect value, requires Value::Object".into()),
                        }
                    }

                    if let Ok(values) = ctx.parent_value.try_downcast_ref::<ErcTokenType>() {
                        return Ok(Some(values.clone().to_field_value()));
                    }

                    Err("unexpected parent value".into())
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

pub trait ResolvableObject: BasicObject {
    // Resolvers that returns single and many objects
    fn resolvers(&self) -> Vec<Field>;

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

    // Connection type includes {type_name}Connection and {type_name}Edge according to relay spec https://relay.dev/graphql/connections.htm
    fn connection_objects(&self) -> Option<Vec<Object>> {
        let edge = EdgeObject::new(self.name().0.to_string(), self.type_name().to_string());
        let connection =
            ConnectionObject::new(self.name().0.to_string(), self.type_name().to_string());

        let mut objects = Vec::new();
        objects.extend(edge.objects());
        objects.extend(connection.objects());

        Some(objects)
    }
}

// Resolves single object queries, returns current object of type type_name (eg "Player")
pub fn resolve_one(
    table_name: &str,
    id_column: &str,
    field_name: &str,
    type_name: &str,
    type_mapping: &TypeMapping,
) -> Field {
    let type_mapping = type_mapping.clone();
    let table_name = table_name.to_owned();
    let id_column = id_column.to_owned();
    let argument = InputValue::new(id_column.to_case(Case::Camel), TypeRef::named_nn(TypeRef::ID));

    Field::new(field_name, TypeRef::named_nn(type_name), move |ctx| {
        let type_mapping = type_mapping.clone();
        let table_name = table_name.to_owned();
        let id_column = id_column.to_owned();

        FieldFuture::new(async move {
            let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
            let id: String =
                extract::<String>(ctx.args.as_index_map(), &id_column.to_case(Case::Camel))?;
            let data = fetch_single_row(&mut conn, &table_name, &id_column, &id).await?;
            let model = value_mapping_from_row(&data, &type_mapping, false)?;
            Ok(Some(Value::Object(model)))
        })
    })
    .argument(argument)
}

// Resolves plural object queries, returns type of {type_name}Connection (eg "PlayerConnection")
pub fn resolve_many(
    table_name: &str,
    id_column: &str,
    field_name: &str,
    type_name: &str,
    type_mapping: &TypeMapping,
) -> Field {
    let type_mapping = type_mapping.clone();
    let table_name = table_name.to_owned();
    let id_column = id_column.to_owned();

    let mut field =
        Field::new(field_name, TypeRef::named(format!("{}Connection", type_name)), move |ctx| {
            let type_mapping = type_mapping.clone();
            let table_name = table_name.to_owned();
            let id_column = id_column.to_owned();

            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let connection = parse_connection_arguments(&ctx)?;
                let keys = parse_keys_argument(&ctx)?;
                let order = parse_order_argument(&ctx);
                let total_count = count_rows(&mut conn, &table_name, &keys, &None).await?;

                let (data, page_info) = fetch_multiple_rows(
                    &mut conn,
                    &table_name,
                    &id_column,
                    &keys,
                    &order,
                    &None,
                    &connection,
                    total_count,
                )
                .await?;
                let results = connection_output(
                    &data,
                    &type_mapping,
                    &order,
                    &id_column,
                    total_count,
                    false,
                    page_info,
                )?;

                Ok(Some(Value::Object(results)))
            })
        });

    field = connection_arguments(field);

    field
}
