use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::Value;
use sqlx::{Pool, Sqlite};

use super::connection::{connection_arguments, connection_output, parse_connection_arguments};
use super::inputs::keys_input::{keys_argument, parse_keys_argument};
use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::mapping::{EVENT_TYPE_MAPPING, SYSTEM_CALL_TYPE_MAPPING};
use crate::query::constants::{EVENT_TABLE, ID_COLUMN};
use crate::query::data::{count_rows, fetch_multiple_rows, fetch_single_row};
use crate::query::value_mapping_from_row;
use crate::utils::extract;

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

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCall", TypeRef::named_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let event_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let syscall_id = extract::<u64>(event_values, "system_call_id")?;
                let data =
                    fetch_single_row(&mut conn, "system_calls", "id", &syscall_id.to_string())
                        .await?;
                let system_call = value_mapping_from_row(&data, &SYSTEM_CALL_TYPE_MAPPING, false)?;

                Ok(Some(Value::Object(system_call)))
            })
        })])
    }
}
