use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use crypto_bigint::U256;
use dojo_types::naming::get_tag;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use dojo_world::contracts::abigen::model::Layout;
use dojo_world::contracts::model::ModelReader;
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::types::Felt;

use super::error::{self, Error};
use crate::error::{ParseError, QueryError};

#[derive(Debug)]
pub struct ModelSQLReader {
    /// Namespace of the model
    namespace: String,
    /// The name of the model
    name: String,
    /// The selector of the model
    selector: Felt,
    /// The class hash of the model
    class_hash: Felt,
    /// The contract address of the model
    contract_address: Felt,
    pool: Pool<Sqlite>,
    packed_size: u32,
    unpacked_size: u32,
    layout: Layout,
}

impl ModelSQLReader {
    pub async fn new(selector: Felt, pool: Pool<Sqlite>) -> Result<Self, Error> {
        let (namespace, name, class_hash, contract_address, packed_size, unpacked_size, layout): (
            String,
            String,
            String,
            String,
            u32,
            u32,
            String,
        ) = sqlx::query_as(
            "SELECT namespace, name, class_hash, contract_address, packed_size, unpacked_size, \
             layout FROM models WHERE id = ?",
        )
        .bind(format!("{:#x}", selector))
        .fetch_one(&pool)
        .await?;

        let class_hash = Felt::from_hex(&class_hash).map_err(error::ParseError::FromStr)?;
        let contract_address =
            Felt::from_hex(&contract_address).map_err(error::ParseError::FromStr)?;

        let layout = serde_json::from_str(&layout).map_err(error::ParseError::FromJsonStr)?;

        Ok(Self {
            namespace,
            name,
            selector,
            class_hash,
            contract_address,
            pool,
            packed_size,
            unpacked_size,
            layout,
        })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ModelReader<Error> for ModelSQLReader {
    fn namespace(&self) -> &str {
        &self.namespace
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn selector(&self) -> Felt {
        self.selector
    }

    fn class_hash(&self) -> Felt {
        self.class_hash
    }

    fn contract_address(&self) -> Felt {
        self.contract_address
    }

    async fn schema(&self) -> Result<Ty, Error> {
        let schema: String = sqlx::query_scalar("SELECT schema FROM models WHERE id = ?")
            .bind(format!("{:#x}", self.selector))
            .fetch_one(&self.pool)
            .await?;

        Ok(serde_json::from_str(&schema).map_err(error::ParseError::FromJsonStr)?)
    }

    async fn packed_size(&self) -> Result<u32, Error> {
        Ok(self.packed_size)
    }

    async fn unpacked_size(&self) -> Result<u32, Error> {
        Ok(self.unpacked_size)
    }

    async fn layout(&self) -> Result<Layout, Error> {
        Ok(self.layout.clone())
    }
}

/// Creates a query that fetches all models and their nested data.
pub fn build_sql_query(
    schemas: &Vec<Ty>,
    table_name: &str,
    entity_relation_column: &str,
    where_clause: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<(String, String), Error> {
    fn collect_columns(table_prefix: &str, path: &str, ty: &Ty, selections: &mut Vec<String>) {
        match ty {
            Ty::Struct(s) => {
                for child in &s.children {
                    let new_path = if path.is_empty() {
                        child.name.clone()
                    } else {
                        format!("{}.{}", path, child.name)
                    };
                    collect_columns(table_prefix, &new_path, &child.ty, selections);
                }
            }
            Ty::Tuple(t) => {
                for (i, child) in t.iter().enumerate() {
                    let new_path =
                        if path.is_empty() { format!("{}", i) } else { format!("{}.{}", path, i) };
                    collect_columns(table_prefix, &new_path, child, selections);
                }
            }
            Ty::Enum(e) => {
                // Add the enum variant column with table prefix and alias
                selections.push(format!(
                    "{}.\"{}\" as \"{}.{}\"",
                    table_prefix, path, table_prefix, path
                ));

                // Add columns for each variant's value (if not empty tuple)
                for option in &e.options {
                    if let Ty::Tuple(t) = &option.ty {
                        if t.is_empty() {
                            continue;
                        }
                    }
                    let variant_path = format!("{}.{}", path, option.name);
                    collect_columns(table_prefix, &variant_path, &option.ty, selections);
                }
            }
            Ty::Array(_) | Ty::Primitive(_) | Ty::ByteArray(_) => {
                selections.push(format!(
                    "{}.\"{}\" as \"{}.{}\"",
                    table_prefix, path, table_prefix, path
                ));
            }
        }
    }

    let mut selections = Vec::new();
    let mut joins = Vec::new();

    // Add base table columns
    selections.push(format!("{}.id", table_name));
    selections.push(format!("{}.keys", table_name));

    // Process each model schema
    for model in schemas {
        let model_table = model.name();
        joins.push(format!(
            "LEFT JOIN [{model_table}] ON {table_name}.id = [{model_table}].{entity_relation_column}",
        ));

        // Collect columns with table prefix
        collect_columns(&model_table, "", model, &mut selections);
    }

    let selections_clause = selections.join(", ");
    let joins_clause = joins.join(" ");

    let mut query = format!("SELECT {} FROM [{}] {}", selections_clause, table_name, joins_clause);

    let mut count_query =
        format!("SELECT COUNT(DISTINCT {}.id) FROM [{}] {}", table_name, table_name, joins_clause);

    if let Some(where_clause) = where_clause {
        query += &format!(" WHERE {}", where_clause);
        count_query += &format!(" WHERE {}", where_clause);
    }

    query += &format!(" ORDER BY {}.event_id DESC", table_name);

    if let Some(limit) = limit {
        query += &format!(" LIMIT {}", limit);
    }

    if let Some(offset) = offset {
        query += &format!(" OFFSET {}", offset);
    }

    Ok((query, count_query))
}

/// Populate the values of a Ty (schema) from SQLite row.
pub fn map_row_to_ty(
    path: &str,
    name: &str,
    ty: &mut Ty,
    // the row that contains non dynamic data for Ty
    row: &SqliteRow,
) -> Result<(), Error> {
    let column_name = if path.is_empty() {
        name
    } else {
        &format!("{}.{}", path, name)
    };

    match ty {
        Ty::Primitive(primitive) => {
            match &primitive {
                Primitive::I8(_) => {
                    let value = row.try_get::<i8, &str>(&column_name)?;
                    primitive.set_i8(Some(value))?;
                }
                Primitive::I16(_) => {
                    let value = row.try_get::<i16, &str>(&column_name)?;
                    primitive.set_i16(Some(value))?;
                }
                Primitive::I32(_) => {
                    let value = row.try_get::<i32, &str>(&column_name)?;
                    primitive.set_i32(Some(value))?;
                }
                Primitive::I64(_) => {
                    let value = row.try_get::<i64, &str>(&column_name)?;
                    primitive.set_i64(Some(value))?;
                }
                Primitive::I128(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    if !hex_str.is_empty() {
                        primitive.set_i128(Some(
                            i128::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                        ))?;
                    }
                }
                Primitive::U8(_) => {
                    let value = row.try_get::<u8, &str>(&column_name)?;
                    primitive.set_u8(Some(value))?;
                }
                Primitive::U16(_) => {
                    let value = row.try_get::<u16, &str>(&column_name)?;
                    primitive.set_u16(Some(value))?;
                }
                Primitive::U32(_) => {
                    let value = row.try_get::<u32, &str>(&column_name)?;
                    primitive.set_u32(Some(value))?;
                }
                Primitive::U64(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    if !hex_str.is_empty() {
                        primitive.set_u64(Some(
                            u64::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                        ))?;
                    }
                }
                Primitive::U128(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    if !hex_str.is_empty() {
                        primitive.set_u128(Some(
                            u128::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                        ))?;
                    }
                }
                Primitive::U256(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    if !hex_str.is_empty() {
                        primitive.set_u256(Some(U256::from_be_hex(hex_str)))?;
                    }
                }
                Primitive::USize(_) => {
                    let value = row.try_get::<u32, &str>(&column_name)?;
                    primitive.set_usize(Some(value))?;
                }
                Primitive::Bool(_) => {
                    let value = row.try_get::<bool, &str>(&column_name)?;
                    primitive.set_bool(Some(value))?;
                }
                Primitive::Felt252(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    if !value.is_empty() {
                        primitive.set_felt252(Some(
                            Felt::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                }
                Primitive::ClassHash(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    if !value.is_empty() {
                        primitive.set_class_hash(Some(
                            Felt::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                }
                Primitive::ContractAddress(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    if !value.is_empty() {
                        primitive.set_contract_address(Some(
                            Felt::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                }
            };
        }
        Ty::Enum(enum_ty) => {
            let option_name = row.try_get::<String, &str>(&column_name)?;
            if !option_name.is_empty() {
                enum_ty.set_option(&option_name)?;
            }

            for option in &mut enum_ty.options {
                if option.name != option_name {
                    continue;
                }

                map_row_to_ty(&column_name, &option.name, &mut option.ty, row)?;
            }
        }
        Ty::Struct(struct_ty) => {
            for member in &mut struct_ty.children {
                map_row_to_ty(&column_name, &member.name, &mut member.ty, row)?;
            }
        }
        Ty::Tuple(ty) => {
            for (i, member) in ty.iter_mut().enumerate() {
                map_row_to_ty(&column_name, &i.to_string(), member, row)?;
            }
        }
        Ty::Array(ty) => {
            let serialized_array = row.try_get::<String, &str>(&column_name)?;

            *ty = serde_json::from_str(&serialized_array).map_err(ParseError::FromJsonStr)?;
        }
        Ty::ByteArray(bytearray) => {
            let value = row.try_get::<String, &str>(&column_name)?;
            *bytearray = value;
        }
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};

    use super::build_sql_query;

    #[test]
    fn struct_ty_to_query() {
        let position = Ty::Struct(Struct {
            name: "Test-Position".into(),
            children: vec![
                dojo_types::schema::Member {
                    name: "player".into(),
                    key: true,
                    ty: Ty::Primitive("ContractAddress".parse().unwrap()),
                },
                dojo_types::schema::Member {
                    name: "vec".into(),
                    key: false,
                    ty: Ty::Struct(Struct {
                        name: "Vec2".into(),
                        children: vec![
                            Member {
                                name: "x".into(),
                                key: false,
                                ty: Ty::Primitive("u32".parse().unwrap()),
                            },
                            Member {
                                name: "y".into(),
                                key: false,
                                ty: Ty::Primitive("u32".parse().unwrap()),
                            },
                        ],
                    }),
                },
                dojo_types::schema::Member {
                    name: "test_everything".into(),
                    key: false,
                    ty: Ty::Array(vec![Ty::Struct(Struct {
                        name: "TestEverything".into(),
                        children: vec![Member {
                            name: "data".into(),
                            key: false,
                            ty: Ty::Tuple(vec![
                                Ty::Array(vec![Ty::Primitive("u32".parse().unwrap())]),
                                Ty::Array(vec![Ty::Array(vec![Ty::Tuple(vec![
                                    Ty::Primitive("u32".parse().unwrap()),
                                    Ty::Struct(Struct {
                                        name: "Vec2".into(),
                                        children: vec![
                                            Member {
                                                name: "x".into(),
                                                key: false,
                                                ty: Ty::Primitive("u32".parse().unwrap()),
                                            },
                                            Member {
                                                name: "y".into(),
                                                key: false,
                                                ty: Ty::Primitive("u32".parse().unwrap()),
                                            },
                                        ],
                                    }),
                                ])])]),
                            ]),
                        }],
                    })]),
                },
            ],
        });

        let player_config = Ty::Struct(Struct {
            name: "Test-PlayerConfig".into(),
            children: vec![
                dojo_types::schema::Member {
                    name: "favorite_item".into(),
                    key: false,
                    ty: Ty::Enum(Enum {
                        name: "Option<u32>".into(),
                        option: None,
                        options: vec![
                            EnumOption { name: "None".into(), ty: Ty::Tuple(vec![]) },
                            EnumOption {
                                name: "Some".into(),
                                ty: Ty::Primitive("u32".parse().unwrap()),
                            },
                        ],
                    }),
                },
                dojo_types::schema::Member {
                    name: "items".into(),
                    key: false,
                    ty: Ty::Array(vec![Ty::Struct(Struct {
                        name: "PlayerItem".into(),
                        children: vec![
                            Member {
                                name: "item_id".into(),
                                key: false,
                                ty: Ty::Primitive("u32".parse().unwrap()),
                            },
                            Member {
                                name: "quantity".into(),
                                key: false,
                                ty: Ty::Primitive("u32".parse().unwrap()),
                            },
                        ],
                    })]),
                },
            ],
        });

        let query = build_sql_query(
            &vec![position, player_config],
            "entities",
            "internal_entity_id",
            None,
            None,
            None,
        )
        .unwrap();

        let expected_query = "SELECT internal_id, internal_entity_id, [player], [vec.x], [vec.y], [test_everything], [favorite_item], [favorite_item.Some], [items] FROM [entities] WHERE entity_id ORDER BY internal_event_id DESC";
        assert_eq!(query.0, expected_query);
    }
}
