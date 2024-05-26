use std::str::FromStr;

use async_trait::async_trait;
use crypto_bigint::U256;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use dojo_world::contracts::abi::model::Layout;
use dojo_world::contracts::model::ModelReader;
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;

use super::error::{self, Error};
use crate::error::{ParseError, QueryError};

pub struct ModelSQLReader {
    /// The name of the model
    name: String,
    /// The class hash of the model
    class_hash: FieldElement,
    /// The contract address of the model
    contract_address: FieldElement,
    pool: Pool<Sqlite>,
    packed_size: u32,
    unpacked_size: u32,
    layout: Layout,
}

impl ModelSQLReader {
    pub async fn new(name: &str, pool: Pool<Sqlite>) -> Result<Self, Error> {
        let (name, class_hash, contract_address, packed_size, unpacked_size, layout): (
            String,
            String,
            String,
            u32,
            u32,
            String,
        ) = sqlx::query_as(
            "SELECT name, class_hash, contract_address, packed_size, unpacked_size, layout FROM \
             models WHERE id = ?",
        )
        .bind(name)
        .fetch_one(&pool)
        .await?;

        let class_hash =
            FieldElement::from_hex_be(&class_hash).map_err(error::ParseError::FromStr)?;
        let contract_address =
            FieldElement::from_hex_be(&contract_address).map_err(error::ParseError::FromStr)?;

        let layout = serde_json::from_str(&layout).map_err(error::ParseError::FromJsonStr)?;

        Ok(Self { name, class_hash, contract_address, pool, packed_size, unpacked_size, layout })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ModelReader<Error> for ModelSQLReader {
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn selector(&self) -> FieldElement {
        // this should never fail
        get_selector_from_name(&self.name).unwrap()
    }

    fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    fn contract_address(&self) -> FieldElement {
        self.contract_address
    }

    async fn schema(&self) -> Result<Ty, Error> {
        // this is temporary until the hash for the model name is precomputed
        let model_selector =
            get_selector_from_name(&self.name).map_err(error::ParseError::NonAsciiName)?;

        let model_members: Vec<SqlModelMember> = sqlx::query_as(
            "SELECT id, model_idx, member_idx, name, type, type_enum, enum_options, key FROM \
             model_members WHERE model_id = ? ORDER BY model_idx ASC, member_idx ASC",
        )
        .bind(format!("{:#x}", model_selector))
        .fetch_all(&self.pool)
        .await?;

        Ok(parse_sql_model_members(&self.name, &model_members))
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
pub fn parse_sql_model_members(model: &str, model_members_all: &[SqlModelMember]) -> Ty {
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
        name: model.into(),
        children: model_members_all
            .iter()
            .filter(|m| m.id == model)
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
    model_schemas: &Vec<Ty>,
    entities_table: &str,
    entity_relation_column: &str,
) -> Result<String, Error> {
    fn parse_struct(
        path: &str,
        schema: &Struct,
        selections: &mut Vec<String>,
        tables: &mut Vec<String>,
    ) {
        for child in &schema.children {
            match &child.ty {
                Ty::Struct(s) => {
                    let table_name = format!("{}${}", path, child.name);
                    parse_struct(&table_name, s, selections, tables);

                    tables.push(table_name);
                }
                _ => {
                    // alias selected columns to avoid conflicts in `JOIN`
                    selections.push(format!(
                        "{}.external_{} AS \"{}.{}\"",
                        path, child.name, path, child.name
                    ));
                }
            }
        }
    }

    let mut global_selections = Vec::new();
    let mut global_tables =
        model_schemas.iter().enumerate().map(|(_, schema)| schema.name()).collect::<Vec<String>>();

    for ty in model_schemas {
        let schema = ty.as_struct().expect("schema should be struct");
        let model_table = &schema.name;
        let mut selections = Vec::new();
        let mut tables = Vec::new();

        parse_struct(model_table, schema, &mut selections, &mut tables);

        global_selections.push(selections.join(", "));
        global_tables.extend(tables);
    }

    // TODO: Fallback to subqueries, SQLite has a max limit of 64 on 'table 'JOIN'
    if global_tables.len() > 64 {
        return Err(QueryError::SqliteJoinLimit.into());
    }

    let selections_clause = global_selections.join(", ");
    let join_clause = global_tables
        .into_iter()
        .map(|table| {
            format!(" JOIN {table} ON {entities_table}.id = {table}.{entity_relation_column}")
        })
        .collect::<Vec<_>>()
        .join(" ");

    Ok(format!(
        "SELECT {entities_table}.id, {entities_table}.keys, {selections_clause} FROM \
         {entities_table}{join_clause}"
    ))
}

/// Populate the values of a Ty (schema) from SQLite row.
pub fn map_row_to_ty(path: &str, struct_ty: &mut Struct, row: &SqliteRow) -> Result<(), Error> {
    for member in struct_ty.children.iter_mut() {
        let column_name = format!("{}.{}", path, member.name);
        match &mut member.ty {
            Ty::Primitive(primitive) => {
                match &primitive {
                    Primitive::Bool(_) => {
                        let value = row.try_get::<bool, &str>(&column_name)?;
                        primitive.set_bool(Some(value))?;
                    }
                    Primitive::USize(_) => {
                        let value = row.try_get::<u32, &str>(&column_name)?;
                        primitive.set_usize(Some(value))?;
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
                    Primitive::Felt252(_) => {
                        let value = row.try_get::<String, &str>(&column_name)?;
                        primitive.set_felt252(Some(
                            FieldElement::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                    Primitive::ClassHash(_) => {
                        let value = row.try_get::<String, &str>(&column_name)?;
                        primitive.set_contract_address(Some(
                            FieldElement::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                    Primitive::ContractAddress(_) => {
                        let value = row.try_get::<String, &str>(&column_name)?;
                        primitive.set_contract_address(Some(
                            FieldElement::from_str(&value).map_err(ParseError::FromStr)?,
                        ))?;
                    }
                };
            }
            Ty::Enum(enum_ty) => {
                let value = row.try_get::<String, &str>(&column_name)?;
                enum_ty.set_option(&value)?;
            }
            Ty::Struct(struct_ty) => {
                let path = [path, &member.name].join("$");
                map_row_to_ty(&path, struct_ty, row)?;
            }
            ty => {
                unimplemented!("unimplemented type_enum: {ty}");
            }
        };
    }

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
                id: "Position".into(),
                name: "x".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Position".into(),
                name: "y".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
        ];

        let expected_ty = Ty::Struct(Struct {
            name: "Position".into(),
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

        assert_eq!(parse_sql_model_members("Position", &model_members), expected_ty);
    }

    #[test]
    fn parse_complex_model_members_to_ty() {
        let model_members = vec![
            SqlModelMember {
                id: "Position".into(),
                name: "name".into(),
                r#type: "felt252".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Position".into(),
                name: "age".into(),
                r#type: "u8".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Position".into(),
                name: "vec".into(),
                r#type: "Vec2".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: "Struct".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Position$vec".into(),
                name: "x".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 1,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Position$vec".into(),
                name: "y".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 1,
                member_idx: 1,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
        ];

        let expected_ty = Ty::Struct(Struct {
            name: "Position".into(),
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

        assert_eq!(parse_sql_model_members("Position", &model_members), expected_ty);
    }

    #[test]
    fn parse_model_members_with_enum_to_ty() {
        let model_members = vec![SqlModelMember {
            id: "Moves".into(),
            name: "direction".into(),
            r#type: "Direction".into(),
            key: false,
            model_idx: 0,
            member_idx: 0,
            type_enum: "Enum".into(),
            enum_options: Some("Up,Down,Left,Right".into()),
        }];

        let expected_ty = Ty::Struct(Struct {
            name: "Moves".into(),
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

        assert_eq!(parse_sql_model_members("Moves", &model_members), expected_ty);
    }

    #[test]
    fn struct_ty_to_query() {
        let ty = Ty::Struct(Struct {
            name: "Position".into(),
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

        let query = build_sql_query(&vec![ty], "entities", "entity_id").unwrap();
        assert_eq!(
            query,
            r#"SELECT entities.id, entities.keys, Position.external_name AS "Position.name", Position.external_age AS "Position.age", Position$vec.external_x AS "Position$vec.x", Position$vec.external_y AS "Position$vec.y" FROM entities JOIN Position ON entities.id = Position.entity_id  JOIN Position$vec ON entities.id = Position$vec.entity_id"#
        );
    }
}
