use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::Value;
use sqlx::{Pool, Sqlite};

use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::mapping::{EVENT_TYPE_MAPPING, SYSTEM_CALL_TYPE_MAPPING};
use crate::query::constants::EVENT_TABLE;
use crate::query::data::fetch_single_row;
use crate::query::value_mapping_from_row;
use crate::utils::extract_value::extract;

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

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCall", TypeRef::named_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let event_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let syscall_id = extract::<i64>(event_values, "system_call_id")?;
                let data =
                    fetch_single_row(&mut conn, "system_calls", "id", &syscall_id.to_string())
                        .await?;
                let system_call = value_mapping_from_row(&data, &SYSTEM_CALL_TYPE_MAPPING, false)?;

                Ok(Some(Value::Object(system_call)))
            })
        })])
    }
}
