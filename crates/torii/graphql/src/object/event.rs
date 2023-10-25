use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::Value;
use sqlx::{Pool, Sqlite};

use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use super::inputs::keys_input::{keys_argument, parse_keys_argument};
use super::{ObjectTrait, TypeMapping};
use crate::mapping::EVENT_TYPE_MAPPING;
use crate::query::constants::{EVENT_TABLE, ID_COLUMN};
use crate::query::data::{count_rows, fetch_multiple_rows};

pub struct EventObject;

impl ObjectTrait for EventObject {
    fn name(&self) -> (&str, &str) {
        ("event", "events")
    }

    fn type_name(&self) -> &str {
        "Event"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &EVENT_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(EVENT_TABLE)
    }

    fn resolve_one(&self) -> Option<Field> {
        None
    }

    fn resolve_many(&self) -> Option<Field> {
        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let keys = parse_keys_argument(&ctx)?;
                    let total_count = count_rows(&mut conn, EVENT_TABLE, &keys, &None).await?;
                    let data = fetch_multiple_rows(
                        &mut conn,
                        EVENT_TABLE,
                        ID_COLUMN,
                        &keys,
                        &None,
                        &None,
                        &connection,
                    )
                    .await?;
                    let results = connection_output(
                        &data,
                        &EVENT_TYPE_MAPPING,
                        &None,
                        ID_COLUMN,
                        total_count,
                        false,
                    )?;

                    Ok(Some(Value::Object(results)))
                })
            },
        );

        field = connection_arguments(field);
        field = keys_argument(field);

        Some(field)
    }
}
