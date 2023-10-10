use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::Value;
use sqlx::{Pool, Sqlite};

use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::mapping::{SYSTEM_CALL_TYPE_MAPPING, SYSTEM_TYPE_MAPPING};
use crate::query::constants::SYSTEM_CALL_TABLE;
use crate::query::data::fetch_single_row;
use crate::query::value_mapping_from_row;
use crate::utils::extract_value::extract;

pub struct SystemCallObject;

impl ObjectTrait for SystemCallObject {
    fn name(&self) -> (&str, &str) {
        ("systemCall", "systemCalls")
    }

    fn type_name(&self) -> &str {
        "SystemCall"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &SYSTEM_CALL_TYPE_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(SYSTEM_CALL_TABLE)
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![Field::new("system", TypeRef::named_nn("System"), |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let syscall_values = ctx.parent_value.try_downcast_ref::<ValueMapping>()?;
                let system_id = extract::<String>(syscall_values, "system_id")?;
                let system = fetch_single_row(&mut conn, "systems", "id", &system_id).await?;
                let result = value_mapping_from_row(&system, &SYSTEM_TYPE_MAPPING, false)?;

                Ok(Some(Value::Object(result)))
            })
        })])
    }
}
