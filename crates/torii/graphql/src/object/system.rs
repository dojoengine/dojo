use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::Value;
use sqlx::{Pool, Sqlite};

use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::mapping::{SYSTEM_CALL_TYPE_MAPPING, SYSTEM_TYPE_MAPPING};
use crate::query::constants::{SYSTEM_CALL_TABLE, SYSTEM_TABLE};
use crate::query::data::fetch_single_row;
use crate::query::value_mapping_from_row;
use crate::utils::extract;

pub struct SystemObject;

impl ObjectTrait for SystemObject {
    fn name(&self) -> (&str, &str) {
        ("system", "systems")
    }

    fn type_name(&self) -> &str {
        "System"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &SYSTEM_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(SYSTEM_TABLE)
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("systemCalls", TypeRef::named_nn_list_nn("SystemCall"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let event_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let syscall_id = extract::<u64>(event_values, "system_call_id")?;
                let data =
                    fetch_single_row(&mut conn, SYSTEM_CALL_TABLE, "id", &syscall_id.to_string())
                        .await?;
                let system_call = value_mapping_from_row(&data, &SYSTEM_CALL_TYPE_MAPPING, false)?;

                Ok(Some(Value::Object(system_call)))
            })
        })])
    }
}
