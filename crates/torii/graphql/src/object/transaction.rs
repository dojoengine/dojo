use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::Value;
use convert_case::{Case, Casing};
use sqlx::{Pool, Sqlite};

use super::{ObjectTrait, TypeMapping};
use crate::constants::{TRANSACTION_NAMES, TRANSACTION_TABLE, TRANSACTION_TYPE_NAME};
use crate::mapping::TRANSACTION_MAPPING;
use crate::query::data::fetch_single_row;
use crate::query::value_mapping_from_row;
use crate::utils::extract;
pub struct TransactionObject;

impl ObjectTrait for TransactionObject {
    fn name(&self) -> (&str, &str) {
        TRANSACTION_NAMES
    }

    fn type_name(&self) -> &str {
        TRANSACTION_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TRANSACTION_MAPPING
    }

    fn table_name(&self) -> Option<&str> {
        Some(TRANSACTION_TABLE)
    }

    fn resolve_one(&self) -> Option<Field> {
        let type_mapping = self.type_mapping().clone();
        let table_name = self.table_name().unwrap().to_string();

        Some(
            Field::new(self.name().0, TypeRef::named_nn(self.type_name()), move |ctx| {
                let type_mapping = type_mapping.clone();
                let table_name = table_name.to_string();

                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let hash =
                        extract::<String>(ctx.args.as_index_map(), &COLUMN.to_case(Case::Camel))?;
                    let data = fetch_single_row(&mut conn, &table_name, COLUMN, &hash).await?;
                    let model = value_mapping_from_row(&data, &type_mapping, false)?;
                    Ok(Some(Value::Object(model)))
                })
            })
            .argument(InputValue::new(
                COLUMN.to_case(Case::Camel),
                TypeRef::named_nn(TypeRef::STRING),
            )),
        )
    }
}

const COLUMN: &str = "transaction_hash";
