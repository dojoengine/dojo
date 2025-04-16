use std::str::FromStr;

use async_trait::async_trait;
use crypto_bigint::U256;
use dojo_types::primitive::{Primitive, PrimitiveError};
use dojo_types::schema::Ty;
use dojo_world::contracts::abigen::model::Layout;
use dojo_world::contracts::model::ModelReader;
use serde_json::Value as JsonValue;
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::types::Felt;

use super::error::{self, Error};
use crate::error::ParseError;

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
#[allow(clippy::too_many_arguments)]
pub fn build_sql_query(
    schemas: &Vec<Ty>,
    table_name: &str,
    model_relation_table: &str,
    entity_relation_column: &str,
    where_clause: Option<&str>,
    having_clause: Option<&str>,
    order_by: Option<&str>,
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
                selections.push(format!("[{table_prefix}].[{path}] as \"{table_prefix}.{path}\"",));

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
                selections.push(format!("[{table_prefix}].[{path}] as \"{table_prefix}.{path}\"",));
            }
        }
    }

    let mut selections = Vec::new();
    let mut joins = Vec::new();

    // Add base table columns
    selections.push(format!("{}.id", table_name));
    selections.push(format!("{}.keys", table_name));
    selections.push(format!("group_concat({model_relation_table}.model_id) as model_ids"));

    // Process each model schema
    for model in schemas {
        let model_table = model.name();
        joins.push(format!(
            "LEFT JOIN [{model_table}] ON {table_name}.id = \
             [{model_table}].{entity_relation_column}",
        ));

        // Collect columns with table prefix
        collect_columns(&model_table, "", model, &mut selections);
    }

    joins.push(format!(
        "JOIN {model_relation_table} ON {table_name}.id = {model_relation_table}.entity_id"
    ));

    let selections_clause = selections.join(", ");
    let joins_clause = joins.join(" ");

    let mut query = format!("SELECT {} FROM [{}] {}", selections_clause, table_name, joins_clause);

    // Include model_ids in the subquery and put WHERE before GROUP BY
    let mut count_query = format!(
        "SELECT COUNT(*) FROM (SELECT {}.id, group_concat({}.model_id) as model_ids FROM [{}] {}",
        table_name, model_relation_table, table_name, joins_clause
    );

    if let Some(where_clause) = where_clause {
        query += &format!(" WHERE {}", where_clause);
        count_query += &format!(" WHERE {}", where_clause);
    }

    query += &format!(" GROUP BY {table_name}.id");
    count_query += &format!(" GROUP BY {table_name}.id");

    if let Some(having_clause) = having_clause {
        query += &format!(" HAVING {}", having_clause);
        count_query += &format!(" HAVING {}", having_clause);
    }

    // Close the subquery
    count_query += ") AS filtered_entities";

    // Use custom order by if provided, otherwise default to event_id DESC
    if let Some(order_clause) = order_by {
        query += &format!(" ORDER BY {}", order_clause);
    } else {
        query += &format!(" ORDER BY {}.event_id DESC", table_name);
    }

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
    let column_name = if path.is_empty() { name } else { &format!("{}.{}", path, name) };

    match ty {
        Ty::Primitive(primitive) => {
            match &primitive {
                Primitive::I8(_) => {
                    let value = row.try_get::<i8, &str>(column_name)?;
                    primitive.set_i8(Some(value))?;
                }
                Primitive::I16(_) => {
                    let value = row.try_get::<i16, &str>(column_name)?;
                    primitive.set_i16(Some(value))?;
                }
                Primitive::I32(_) => {
                    let value = row.try_get::<i32, &str>(column_name)?;
                    primitive.set_i32(Some(value))?;
                }
                Primitive::I64(_) => {
                    let value = row.try_get::<i64, &str>(column_name)?;
                    primitive.set_i64(Some(value))?;
                }
                Primitive::I128(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    primitive.set_i128(Some(
                        u128::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?
                            as i128,
                    ))?;
                }
                Primitive::U8(_) => {
                    let value = row.try_get::<u8, &str>(column_name)?;
                    primitive.set_u8(Some(value))?;
                }
                Primitive::U16(_) => {
                    let value = row.try_get::<u16, &str>(column_name)?;
                    primitive.set_u16(Some(value))?;
                }
                Primitive::U32(_) => {
                    let value = row.try_get::<u32, &str>(column_name)?;
                    primitive.set_u32(Some(value))?;
                }
                Primitive::U64(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    if !hex_str.is_empty() {
                        primitive.set_u64(Some(
                            u64::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                        ))?;
                    }
                }
                Primitive::U128(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    if !hex_str.is_empty() {
                        primitive.set_u128(Some(
                            u128::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                        ))?;
                    }
                }
                Primitive::U256(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    let hex_str = value.trim_start_matches("0x");

                    if !hex_str.is_empty() {
                        primitive.set_u256(Some(U256::from_be_hex(hex_str)))?;
                    }
                }
                Primitive::Bool(_) => {
                    let value = row.try_get::<bool, &str>(column_name)?;
                    primitive.set_bool(Some(value))?;
                }
                Primitive::Felt252(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    if !value.is_empty() {
                        primitive.set_felt252(Some(
                            Felt::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                }
                Primitive::ClassHash(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    if !value.is_empty() {
                        primitive.set_class_hash(Some(
                            Felt::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                }
                Primitive::ContractAddress(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    if !value.is_empty() {
                        primitive.set_contract_address(Some(
                            Felt::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                }
                Primitive::EthAddress(_) => {
                    let value = row.try_get::<String, &str>(column_name)?;
                    if !value.is_empty() {
                        primitive.set_eth_address(Some(
                            Felt::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                }
            };
        }
        Ty::Enum(enum_ty) => {
            let option_name = row.try_get::<String, &str>(column_name)?;
            if !option_name.is_empty() {
                enum_ty.set_option(&option_name)?;
            }

            for option in &mut enum_ty.options {
                if option.name != option_name {
                    continue;
                }

                map_row_to_ty(column_name, &option.name, &mut option.ty, row)?;
            }
        }
        Ty::Struct(struct_ty) => {
            for member in &mut struct_ty.children {
                map_row_to_ty(column_name, &member.name, &mut member.ty, row)?;
            }
        }
        Ty::Tuple(ty) => {
            for (i, member) in ty.iter_mut().enumerate() {
                map_row_to_ty(column_name, &i.to_string(), member, row)?;
            }
        }
        Ty::Array(ty) => {
            let schema = ty[0].clone();
            let serialized_array = row.try_get::<String, &str>(column_name)?;
            if serialized_array.is_empty() {
                *ty = vec![];
                return Ok(());
            }

            let values: Vec<JsonValue> =
                serde_json::from_str(&serialized_array).map_err(ParseError::FromJsonStr)?;
            *ty = values
                .iter()
                .map(|v| {
                    let mut ty = schema.clone();
                    ty.from_json_value(v.clone())?;
                    Result::<_, PrimitiveError>::Ok(ty)
                })
                .collect::<Result<Vec<Ty>, _>>()?;
        }
        Ty::ByteArray(bytearray) => {
            let value = row.try_get::<String, &str>(column_name)?;
            *bytearray = value;
        }
    };

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn fetch_entities(
    pool: &Pool<sqlx::Sqlite>,
    schemas: &[Ty],
    table_name: &str,
    model_relation_table: &str,
    entity_relation_column: &str,
    where_clause: Option<&str>,
    having_clause: Option<&str>,
    order_by: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
    bind_values: Vec<String>,
) -> Result<(Vec<sqlx::sqlite::SqliteRow>, u32), Error> {
    // Helper function to collect columns (existing implementation)
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
                selections.push(format!("[{table_prefix}].[{path}] as \"{table_prefix}.{path}\"",));

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
                selections.push(format!("[{table_prefix}].[{path}] as \"{table_prefix}.{path}\"",));
            }
        }
    }

    const MAX_JOINS: usize = 64;
    let schema_chunks = schemas.chunks(MAX_JOINS);
    let mut total_count = 0;
    let mut all_rows = Vec::new();

    for chunk in schema_chunks {
        let mut selections = Vec::new();
        let mut joins = Vec::new();

        // Add base table columns
        selections.push(format!("{}.id", table_name));
        selections.push(format!("{}.keys", table_name));
        selections.push(format!("group_concat({model_relation_table}.model_id) as model_ids"));

        // Process each model schema in the chunk
        for model in chunk {
            let model_table = model.name();
            joins.push(format!(
                "LEFT JOIN [{model_table}] ON {table_name}.id = \
                 [{model_table}].{entity_relation_column}"
            ));
            collect_columns(&model_table, "", model, &mut selections);
        }

        joins.push(format!(
            "JOIN {model_relation_table} ON {table_name}.id = {model_relation_table}.entity_id"
        ));

        let selections_clause = selections.join(", ");
        let joins_clause = joins.join(" ");

        // Build count query
        let count_query = format!(
            "SELECT COUNT(*) FROM (SELECT {}.id, group_concat({}.model_id) as model_ids FROM [{}] \
             {} {} GROUP BY {}.id {})",
            table_name,
            model_relation_table,
            table_name,
            joins_clause,
            where_clause.map_or(String::new(), |w| format!(" WHERE {}", w)),
            table_name,
            having_clause.map_or(String::new(), |h| format!(" HAVING {}", h))
        );

        // Execute count query
        let mut count_stmt = sqlx::query_scalar(&count_query);
        for value in &bind_values {
            count_stmt = count_stmt.bind(value);
        }
        let chunk_count: u32 = count_stmt.fetch_one(pool).await?;
        total_count += chunk_count;

        if chunk_count > 0 {
            // Build main query
            let mut query =
                format!("SELECT {} FROM [{}] {}", selections_clause, table_name, joins_clause);

            if let Some(where_clause) = where_clause {
                query += &format!(" WHERE {}", where_clause);
            }

            query += &format!(" GROUP BY {table_name}.id");

            if let Some(having_clause) = having_clause {
                query += &format!(" HAVING {}", having_clause);
            }

            if let Some(order_clause) = order_by {
                query += &format!(" ORDER BY {}", order_clause);
            } else {
                query += &format!(" ORDER BY {}.event_id DESC", table_name);
            }

            if let Some(limit) = limit {
                query += &format!(" LIMIT {}", limit);
            }

            if let Some(offset) = offset {
                query += &format!(" OFFSET {}", offset);
            }

            // Execute main query
            let mut stmt = sqlx::query(&query);
            for value in &bind_values {
                stmt = stmt.bind(value);
            }
            let chunk_rows = stmt.fetch_all(pool).await?;
            all_rows.extend(chunk_rows);
        }
    }

    Ok((all_rows, total_count))
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
            "entity_model",
            "internal_entity_id",
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let expected_query =
            "SELECT entities.id, entities.keys, group_concat(entity_model.model_id) as model_ids, \
             [Test-Position].[player] as \"Test-Position.player\", [Test-Position].[vec.x] as \
             \"Test-Position.vec.x\", [Test-Position].[vec.y] as \"Test-Position.vec.y\", \
             [Test-Position].[test_everything] as \"Test-Position.test_everything\", \
             [Test-PlayerConfig].[favorite_item] as \"Test-PlayerConfig.favorite_item\", \
             [Test-PlayerConfig].[favorite_item.Some] as \
             \"Test-PlayerConfig.favorite_item.Some\", [Test-PlayerConfig].[items] as \
             \"Test-PlayerConfig.items\" FROM [entities] LEFT JOIN [Test-Position] ON entities.id \
             = [Test-Position].internal_entity_id LEFT JOIN [Test-PlayerConfig] ON entities.id = \
             [Test-PlayerConfig].internal_entity_id JOIN entity_model ON entities.id = \
             entity_model.entity_id GROUP BY entities.id ORDER BY entities.event_id DESC";
        assert_eq!(query.0, expected_query);
    }
}
