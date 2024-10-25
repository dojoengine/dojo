use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use crypto_bigint::U256;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use dojo_world::contracts::abi::model::Layout;
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
        let model_members: Vec<SqlModelMember> = sqlx::query_as(
            "SELECT id, model_idx, member_idx, name, type, type_enum, enum_options, key FROM \
             model_members WHERE model_id = ? ORDER BY model_idx ASC, member_idx ASC",
        )
        .bind(format!("{:#x}", self.selector))
        .fetch_all(&self.pool)
        .await?;

        Ok(parse_sql_model_members(&self.namespace, &self.name, &model_members))
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

#[allow(unused)]
#[derive(Debug, sqlx::FromRow)]
pub struct SqlModelMember {
    id: String,
    model_idx: u32,
    member_idx: u32,
    name: String,
    r#type: String,
    type_enum: String,
    enum_options: Option<String>,
    key: bool,
}

// assume that the model members are sorted by model_idx and member_idx
// `id` is the type id of the model member
/// A helper function to parse the model members from sql table to `Ty`
pub fn parse_sql_model_members(
    namespace: &str,
    model: &str,
    model_members_all: &[SqlModelMember],
) -> Ty {
    fn parse_sql_member(member: &SqlModelMember, model_members_all: &[SqlModelMember]) -> Ty {
        match member.type_enum.as_str() {
            "Primitive" => Ty::Primitive(member.r#type.parse().unwrap()),
            "ByteArray" => Ty::ByteArray("".to_string()),
            "Struct" => {
                let children = model_members_all
                    .iter()
                    .filter(|m| m.id == format!("{}${}", member.id, member.name))
                    .map(|child| Member {
                        key: child.key,
                        name: child.name.to_owned(),
                        ty: parse_sql_member(child, model_members_all),
                    })
                    .collect::<Vec<Member>>();

                Ty::Struct(Struct { name: member.r#type.clone(), children })
            }
            "Enum" => {
                let options = member
                    .enum_options
                    .as_ref()
                    .expect("qed; enum_options should exist")
                    .split(',')
                    .map(|s| {
                        let member = if let Some(member) = model_members_all.iter().find(|m| {
                            m.id == format!("{}${}", member.id, member.name) && m.name == s
                        }) {
                            parse_sql_member(member, model_members_all)
                        } else {
                            Ty::Tuple(vec![])
                        };

                        EnumOption { name: s.to_owned(), ty: member }
                    })
                    .collect::<Vec<EnumOption>>();

                Ty::Enum(Enum { option: None, name: member.r#type.clone(), options })
            }
            "Tuple" => {
                let children = model_members_all
                    .iter()
                    .filter(|m| m.id == format!("{}${}", member.id, member.name))
                    .map(|child| Member {
                        key: child.key,
                        name: child.name.to_owned(),
                        ty: parse_sql_member(child, model_members_all),
                    })
                    .collect::<Vec<Member>>();

                Ty::Tuple(children.into_iter().map(|m| m.ty).collect())
            }
            "Array" => {
                let children = model_members_all
                    .iter()
                    .filter(|m| m.id == format!("{}${}", member.id, member.name))
                    .map(|child| Member {
                        key: child.key,
                        name: child.name.to_owned(),
                        ty: parse_sql_member(child, model_members_all),
                    })
                    .collect::<Vec<Member>>();

                Ty::Array(children.into_iter().map(|m| m.ty).collect())
            }
            ty => {
                unimplemented!("unimplemented type_enum: {ty}");
            }
        }
    }

    Ty::Struct(Struct {
        name: format!("{}-{}", namespace, model),
        children: model_members_all
            .iter()
            .filter(|m| m.id == format!("{}-{}", namespace, model))
            .map(|m| Member {
                key: m.key,
                name: m.name.to_owned(),
                ty: parse_sql_member(m, model_members_all),
            })
            .collect::<Vec<Member>>(),
    })
    // parse_sql_model_members_impl(model, model, model_members_all)
}

/// Creates a query that fetches all models and their nested data.
pub fn build_sql_query(
    schemas: &Vec<Ty>,
    entities_table: &str,
    entity_relation_column: &str,
    where_clause: Option<&str>,
    where_clause_arrays: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<(String, HashMap<String, String>, String), Error> {
    #[derive(Default)]
    struct TableInfo {
        table_name: String,
        parent_table: Option<String>,
        is_optional: bool,
        depth: usize, // Track nesting depth for proper ordering
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_ty(
        path: &str,
        name: &str,
        ty: &Ty,
        selections: &mut Vec<String>,
        tables: &mut Vec<TableInfo>,
        arrays_queries: &mut HashMap<String, (Vec<String>, Vec<TableInfo>)>,
        parent_is_optional: bool,
        depth: usize,
    ) {
        match &ty {
            Ty::Struct(s) => {
                let table_name =
                    if path.is_empty() { name.to_string() } else { format!("{}${}", path, name) };

                tables.push(TableInfo {
                    table_name: table_name.clone(),
                    parent_table: if path.is_empty() { None } else { Some(path.to_string()) },
                    is_optional: parent_is_optional,
                    depth,
                });

                for child in &s.children {
                    parse_ty(
                        &table_name,
                        &child.name,
                        &child.ty,
                        selections,
                        tables,
                        arrays_queries,
                        parent_is_optional,
                        depth + 1,
                    );
                }
            }
            Ty::Tuple(t) => {
                let table_name = format!("{}${}", path, name);

                tables.push(TableInfo {
                    table_name: table_name.clone(),
                    parent_table: Some(path.to_string()),
                    is_optional: parent_is_optional,
                    depth,
                });

                for (i, child) in t.iter().enumerate() {
                    parse_ty(
                        &table_name,
                        &format!("_{}", i),
                        child,
                        selections,
                        tables,
                        arrays_queries,
                        parent_is_optional,
                        depth + 1,
                    );
                }
            }
            Ty::Array(t) => {
                let table_name = format!("{}${}", path, name);
                let is_optional = true;

                let mut array_selections = Vec::new();
                let mut array_tables = vec![TableInfo {
                    table_name: table_name.clone(),
                    parent_table: Some(path.to_string()),
                    is_optional: true,
                    depth,
                }];

                parse_ty(
                    &table_name,
                    "data",
                    &t[0],
                    &mut array_selections,
                    &mut array_tables,
                    arrays_queries,
                    is_optional,
                    depth + 1,
                );

                arrays_queries.insert(table_name, (array_selections, array_tables));
            }
            Ty::Enum(e) => {
                let table_name = format!("{}${}", path, name);
                let is_optional = true;

                let mut is_typed = false;
                for option in &e.options {
                    if let Ty::Tuple(t) = &option.ty {
                        if t.is_empty() {
                            continue;
                        }
                    }

                    parse_ty(
                        &table_name,
                        &option.name,
                        &option.ty,
                        selections,
                        tables,
                        arrays_queries,
                        is_optional,
                        depth + 1,
                    );
                    is_typed = true;
                }

                selections.push(format!("[{}].external_{} AS \"{}.{}\"", path, name, path, name));
                if is_typed {
                    tables.push(TableInfo {
                        table_name,
                        parent_table: Some(path.to_string()),
                        is_optional: parent_is_optional || is_optional,
                        depth,
                    });
                }
            }
            _ => {
                selections.push(format!("[{}].external_{} AS \"{}.{}\"", path, name, path, name));
            }
        }
    }

    let mut global_selections = Vec::new();
    let mut global_tables = Vec::new();
    let mut arrays_queries: HashMap<String, (Vec<String>, Vec<TableInfo>)> = HashMap::new();

    for model in schemas {
        parse_ty(
            "",
            &model.name(),
            model,
            &mut global_selections,
            &mut global_tables,
            &mut arrays_queries,
            false,
            0,
        );
    }

    if global_tables.len() > 64 {
        return Err(QueryError::SqliteJoinLimit.into());
    }

    // Sort tables by depth to ensure proper join order
    global_tables.sort_by_key(|table| table.depth);

    let selections_clause = global_selections.join(", ");
    let join_clause = global_tables
        .iter()
        .map(|table| {
            let join_type = if table.is_optional { "LEFT JOIN" } else { "JOIN" };
            let join_condition = if table.parent_table.is_none() {
                format!("{entities_table}.id = [{}].{entity_relation_column}", table.table_name)
            } else {
                format!(
                    "[{}].full_array_id = [{}].full_array_id",
                    table.table_name,
                    table.parent_table.as_ref().unwrap()
                )
            };
            format!(" {join_type} [{}] ON {join_condition}", table.table_name)
        })
        .collect::<Vec<_>>()
        .join(" ");

    let mut formatted_arrays_queries: HashMap<String, String> = arrays_queries
        .into_iter()
        .map(|(table, (selections, mut tables))| {
            let mut selections_clause = selections.join(", ");
            if !selections_clause.is_empty() {
                selections_clause = format!(", {}", selections_clause);
            }

            // Sort array tables by depth
            tables.sort_by_key(|table| table.depth);

            let join_clause = tables
                .iter()
                .enumerate()
                .map(|(idx, table)| {
                    if idx == 0 {
                        format!(
                            " JOIN [{}] ON {entities_table}.id = [{}].{entity_relation_column}",
                            table.table_name, table.table_name
                        )
                    } else {
                        let join_type = if table.is_optional { "LEFT JOIN" } else { "JOIN" };
                        format!(
                            " {join_type} [{}] ON [{}].full_array_id = [{}].full_array_id",
                            table.table_name,
                            table.table_name,
                            table.parent_table.as_ref().unwrap()
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            (
                table,
                format!(
                    "SELECT {entities_table}.id, {entities_table}.keys{selections_clause} FROM \
                     {entities_table}{join_clause}",
                ),
            )
        })
        .collect();

    let mut query = format!(
        "SELECT {entities_table}.id, {entities_table}.keys, {selections_clause} FROM \
         {entities_table}{join_clause}"
    );
    let mut count_query =
        format!("SELECT COUNT({entities_table}.id) FROM {entities_table}{join_clause}");

    if let Some(where_clause) = where_clause {
        query += &format!(" WHERE {}", where_clause);
        count_query += &format!(" WHERE {}", where_clause);
    }
    query += &format!(" ORDER BY {entities_table}.event_id DESC");

    if let Some(limit) = limit {
        query += &format!(" LIMIT {}", limit);
    }

    if let Some(offset) = offset {
        query += &format!(" OFFSET {}", offset);
    }

    if let Some(where_clause_arrays) = where_clause_arrays {
        for (_, formatted_query) in formatted_arrays_queries.iter_mut() {
            *formatted_query = format!("{} WHERE {}", formatted_query, where_clause_arrays);
        }
    }

    Ok((query, formatted_arrays_queries, count_query))
}

/// Populate the values of a Ty (schema) from SQLite row.
pub fn map_row_to_ty(
    path: &str,
    name: &str,
    ty: &mut Ty,
    // the row that contains non dynamic data for Ty
    row: &SqliteRow,
    // a hashmap where keys are the paths for the model
    // arrays and values are the rows mapping to each element
    // in the array
    arrays_rows: &HashMap<String, Vec<SqliteRow>>,
) -> Result<(), Error> {
    let column_name = format!("{}.{}", path, name);

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
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");
                    primitive.set_i64(Some(
                        i64::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                    ))?;
                }
                Primitive::I128(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");
                    primitive.set_i128(Some(
                        i128::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                    ))?;
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
                    primitive.set_u64(Some(
                        u64::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                    ))?;
                }
                Primitive::U128(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");
                    primitive.set_u128(Some(
                        u128::from_str_radix(hex_str, 16).map_err(ParseError::ParseIntError)?,
                    ))?;
                }
                Primitive::U256(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    let hex_str = value.trim_start_matches("0x");
                    primitive.set_u256(Some(U256::from_be_hex(hex_str)))?;
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
                    primitive
                        .set_felt252(Some(Felt::from_str(&value).map_err(ParseError::FromStr)?))?;
                }
                Primitive::ClassHash(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    primitive.set_class_hash(Some(
                        Felt::from_str(&value).map_err(ParseError::FromStr)?,
                    ))?;
                }
                Primitive::ContractAddress(_) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    primitive.set_contract_address(Some(
                        Felt::from_str(&value).map_err(ParseError::FromStr)?,
                    ))?;
                }
            };
        }
        Ty::Enum(enum_ty) => {
            let option_name = row.try_get::<String, &str>(&column_name)?;
            enum_ty.set_option(&option_name)?;

            let path = [path, name].join("$");
            for option in &mut enum_ty.options {
                if option.name != option_name {
                    continue;
                }

                map_row_to_ty(&path, &option.name, &mut option.ty, row, arrays_rows)?;
            }
        }
        Ty::Struct(struct_ty) => {
            // struct can be the main entrypoint to our model schema
            // so we dont format the table name if the path is empty
            let path =
                if path.is_empty() { struct_ty.name.clone() } else { [path, name].join("$") };

            for member in &mut struct_ty.children {
                map_row_to_ty(&path, &member.name, &mut member.ty, row, arrays_rows)?;
            }
        }
        Ty::Tuple(ty) => {
            let path = [path, name].join("$");

            for (i, member) in ty.iter_mut().enumerate() {
                map_row_to_ty(&path, &format!("_{}", i), member, row, arrays_rows)?;
            }
        }
        Ty::Array(ty) => {
            let path = [path, name].join("$");
            // filter by entity id in case we have multiple entities
            let rows = arrays_rows
                .get(&path)
                .expect("qed; rows should exist")
                .iter()
                .filter(|array_row| array_row.get::<String, _>("id") == row.get::<String, _>("id"))
                .collect::<Vec<_>>();

            // map each row to the ty of the array
            let tys = rows
                .iter()
                .map(|row| {
                    let mut ty = ty[0].clone();
                    map_row_to_ty(&path, "data", &mut ty, row, arrays_rows).map(|_| ty)
                })
                .collect::<Result<Vec<_>, _>>()?;

            *ty = tys;
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

    use super::{build_sql_query, SqlModelMember};
    use crate::model::parse_sql_model_members;

    #[test]
    fn parse_simple_model_members_to_ty() {
        let model_members = vec![
            SqlModelMember {
                id: "Test-Position".into(),
                name: "x".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-Position".into(),
                name: "y".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-PlayerConfig".into(),
                name: "name".into(),
                r#type: "ByteArray".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: "ByteArray".into(),
                enum_options: None,
            },
        ];

        let expected_position = Ty::Struct(Struct {
            name: "Test-Position".into(),
            children: vec![
                dojo_types::schema::Member {
                    name: "x".into(),
                    key: false,
                    ty: Ty::Primitive("u256".parse().unwrap()),
                },
                dojo_types::schema::Member {
                    name: "y".into(),
                    key: false,
                    ty: Ty::Primitive("u256".parse().unwrap()),
                },
            ],
        });

        let expected_player_config = Ty::Struct(Struct {
            name: "Test-PlayerConfig".into(),
            children: vec![dojo_types::schema::Member {
                name: "name".into(),
                key: false,
                ty: Ty::ByteArray("".to_string()),
            }],
        });

        assert_eq!(parse_sql_model_members("Test", "Position", &model_members), expected_position);
        assert_eq!(
            parse_sql_model_members("Test", "PlayerConfig", &model_members),
            expected_player_config
        );
    }

    #[test]
    fn parse_complex_model_members_to_ty() {
        let model_members = vec![
            SqlModelMember {
                id: "Test-Position".into(),
                name: "name".into(),
                r#type: "felt252".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-Position".into(),
                name: "age".into(),
                r#type: "u8".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-Position".into(),
                name: "vec".into(),
                r#type: "Vec2".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Struct".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-Position$vec".into(),
                name: "x".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 1,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-Position$vec".into(),
                name: "y".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 1,
                member_idx: 1,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-PlayerConfig".into(),
                name: "favorite_item".into(),
                r#type: "Option<u32>".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: "Enum".into(),
                enum_options: Some("None,Some".into()),
            },
            SqlModelMember {
                id: "Test-PlayerConfig".into(),
                name: "items".into(),
                r#type: "Array<PlayerItem>".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Array".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-PlayerConfig$items".into(),
                name: "data".into(),
                r#type: "PlayerItem".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Struct".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-PlayerConfig$items$data".into(),
                name: "item_id".into(),
                r#type: "u32".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-PlayerConfig$items$data".into(),
                name: "quantity".into(),
                r#type: "u32".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-PlayerConfig$favorite_item".into(),
                name: "Some".into(),
                r#type: "u32".into(),
                key: false,
                model_idx: 1,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Test-PlayerConfig$favorite_item".into(),
                name: "option".into(),
                r#type: "Option<u32>".into(),
                key: false,
                model_idx: 1,
                member_idx: 0,
                type_enum: "Enum".into(),
                enum_options: Some("None,Some".into()),
            },
        ];

        let expected_position = Ty::Struct(Struct {
            name: "Test-Position".into(),
            children: vec![
                dojo_types::schema::Member {
                    name: "name".into(),
                    key: false,
                    ty: Ty::Primitive("felt252".parse().unwrap()),
                },
                dojo_types::schema::Member {
                    name: "age".into(),
                    key: false,
                    ty: Ty::Primitive("u8".parse().unwrap()),
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
                                ty: Ty::Primitive("u256".parse().unwrap()),
                            },
                            Member {
                                name: "y".into(),
                                key: false,
                                ty: Ty::Primitive("u256".parse().unwrap()),
                            },
                        ],
                    }),
                },
            ],
        });

        let expected_player_config = Ty::Struct(Struct {
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

        assert_eq!(parse_sql_model_members("Test", "Position", &model_members), expected_position);
        assert_eq!(
            parse_sql_model_members("Test", "PlayerConfig", &model_members),
            expected_player_config
        );
    }

    #[test]
    fn parse_model_members_with_enum_to_ty() {
        let model_members = vec![SqlModelMember {
            id: "Test-Moves".into(),
            name: "direction".into(),
            r#type: "Direction".into(),
            key: false,
            model_idx: 0,
            member_idx: 0,
            type_enum: "Enum".into(),
            enum_options: Some("Up,Down,Left,Right".into()),
        }];

        let expected_ty = Ty::Struct(Struct {
            name: "Test-Moves".into(),
            children: vec![dojo_types::schema::Member {
                name: "direction".into(),
                key: false,
                ty: Ty::Enum(Enum {
                    name: "Direction".into(),
                    option: None,
                    options: vec![
                        EnumOption { name: "Up".into(), ty: Ty::Tuple(vec![]) },
                        EnumOption { name: "Down".into(), ty: Ty::Tuple(vec![]) },
                        EnumOption { name: "Left".into(), ty: Ty::Tuple(vec![]) },
                        EnumOption { name: "Right".into(), ty: Ty::Tuple(vec![]) },
                    ],
                }),
            }],
        });

        assert_eq!(parse_sql_model_members("Test", "Moves", &model_members), expected_ty);
    }

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
            "entity_id",
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let expected_query =
            "SELECT entities.id, entities.keys, [Test-Position].external_player AS \
             \"Test-Position.player\", [Test-Position$vec].external_x AS \"Test-Position$vec.x\", \
             [Test-Position$vec].external_y AS \"Test-Position$vec.y\", \
             [Test-PlayerConfig$favorite_item].external_Some AS \
             \"Test-PlayerConfig$favorite_item.Some\", [Test-PlayerConfig].external_favorite_item \
             AS \"Test-PlayerConfig.favorite_item\" FROM entities JOIN [Test-Position$vec] ON \
             entities.id = [Test-Position$vec].entity_id  JOIN [Test-Position] ON entities.id = \
             [Test-Position].entity_id  JOIN [Test-PlayerConfig$favorite_item] ON entities.id = \
             [Test-PlayerConfig$favorite_item].entity_id  JOIN [Test-PlayerConfig] ON entities.id \
             = [Test-PlayerConfig].entity_id ORDER BY entities.event_id DESC";
        // todo: completely tests arrays
        assert_eq!(query.0, expected_query);
    }
}
