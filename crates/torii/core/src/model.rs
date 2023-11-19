use std::str::FromStr;

use async_trait::async_trait;
use crypto_bigint::U256;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use dojo_world::contracts::model::ModelReader;
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use starknet::core::types::FieldElement;

use super::error::{self, Error};
use crate::error::ParseError;

pub struct ModelSQLReader {
    /// The name of the model
    name: String,
    /// The class hash of the model
    class_hash: FieldElement,
    pool: Pool<Sqlite>,
    packed_size: FieldElement,
    unpacked_size: FieldElement,
    layout: Vec<FieldElement>,
}

impl ModelSQLReader {
    pub async fn new(name: &str, pool: Pool<Sqlite>) -> Result<Self, Error> {
        let (name, class_hash, packed_size, unpacked_size, layout): (
            String,
            String,
            u32,
            u32,
            String,
        ) = sqlx::query_as(
            "SELECT name, class_hash, packed_size, unpacked_size, layout FROM models WHERE id = ?",
        )
        .bind(name)
        .fetch_one(&pool)
        .await?;

        let class_hash =
            FieldElement::from_hex_be(&class_hash).map_err(error::ParseError::FromStr)?;
        let packed_size = FieldElement::from(packed_size);
        let unpacked_size = FieldElement::from(unpacked_size);

        let layout = hex::decode(layout).unwrap();
        let layout = layout.iter().map(|e| FieldElement::from(*e)).collect();

        Ok(Self { name, class_hash, pool, packed_size, unpacked_size, layout })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ModelReader<Error> for ModelSQLReader {
    fn class_hash(&self) -> FieldElement {
        self.class_hash
    }

    async fn schema(&self) -> Result<Ty, Error> {
        let model_members: Vec<SqlModelMember> = sqlx::query_as(
            "SELECT id, model_idx, member_idx, name, type, type_enum, enum_options, key FROM \
             model_members WHERE model_id = ? ORDER BY model_idx ASC, member_idx ASC",
        )
        .bind(self.name.clone())
        .fetch_all(&self.pool)
        .await?;

        Ok(parse_sql_model_members(&self.name, &model_members))
    }

    async fn packed_size(&self) -> Result<FieldElement, Error> {
        Ok(self.packed_size)
    }

    async fn unpacked_size(&self) -> Result<FieldElement, Error> {
        Ok(self.unpacked_size)
    }

    async fn layout(&self) -> Result<Vec<FieldElement>, Error> {
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
    fn parse_sql_model_members_impl(path: &str, model_members_all: &[SqlModelMember]) -> Ty {
        let children = model_members_all
            .iter()
            .filter(|member| member.id == path)
            .map(|child| match child.type_enum.as_ref() {
                "Primitive" => Member {
                    key: child.key,
                    name: child.name.to_owned(),
                    ty: Ty::Primitive(child.r#type.parse().unwrap()),
                },

                "Struct" => Member {
                    key: child.key,
                    name: child.name.to_owned(),
                    ty: parse_sql_model_members_impl(
                        &format!("{}${}", child.id, child.r#type),
                        model_members_all,
                    ),
                },

                "Enum" => Member {
                    key: child.key,
                    name: child.name.to_owned(),
                    ty: Ty::Enum(Enum {
                        option: None,
                        name: child.r#type.to_owned(),
                        options: child
                            .enum_options
                            .as_ref()
                            .expect("qed; enum_options should exist")
                            .split(',')
                            .map(|s| EnumOption { name: s.to_owned(), ty: Ty::Tuple(vec![]) })
                            .collect::<Vec<_>>(),
                    }),
                },

                ty => {
                    unimplemented!("unimplemented type_enum: {ty}");
                }
            })
            .collect::<Vec<Member>>();

        // refer to the sql table for `model_members`
        let model_name = path.split('$').last().unwrap_or(path);

        Ty::Struct(Struct { name: model_name.to_owned(), children })
    }

    parse_sql_model_members_impl(model, model_members_all)
}

/// A helper function to build a model query including all nested structs
pub fn build_sql_model_query(schema: &Struct) -> String {
    fn build_sql_model_query_impl(
        path: &str,
        schema: &Struct,
        selections: &mut Vec<String>,
        tables: &mut Vec<String>,
    ) {
        for child in &schema.children {
            match &child.ty {
                Ty::Struct(s) => {
                    let table_name = format!("{}${}", path, s.name);
                    build_sql_model_query_impl(&table_name, s, selections, tables);

                    tables.push(table_name);
                }
                _ => {
                    // alias selected columns to avoid conflicts in JOIN
                    selections.push(format!(
                        "{}.external_{} AS \"{}.{}\"",
                        path, child.name, path, child.name
                    ));
                }
            }
        }
    }

    let model_table = &schema.name;
    let mut selections = Vec::new();
    let mut tables = Vec::new();

    build_sql_model_query_impl(model_table, schema, &mut selections, &mut tables);
    let selections_clause = selections.join(", ");
    let join_clause = tables
        .into_iter()
        .map(|table| {
            format!(" LEFT JOIN {} ON {}.entity_id = {}.entity_id", table, model_table, table)
        })
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        "SELECT {selections_clause} FROM {model_table}{join_clause} ORDER BY \
         {model_table}.event_id"
    )
}

/// Converts SQLite rows into a vector of `Ty` based on a specified schema.
pub fn map_rows_to_tys(schema: &Struct, rows: &[SqliteRow]) -> Result<Vec<Ty>, Error> {
    fn populate_struct_from_row(
        path: &str,
        struct_ty: &mut Struct,
        row: &SqliteRow,
    ) -> Result<(), Error> {
        for child in struct_ty.children.iter_mut() {
            let column_name = format!("{}.{}", path, child.name);
            match &mut child.ty {
                Ty::Primitive(p) => {
                    match &p {
                        Primitive::Bool(_) => {
                            let value = row.try_get::<i64, &str>(&column_name)?;
                            p.set_bool(Some(value == 1))?;
                        }
                        Primitive::USize(_) => {
                            let value = row.try_get::<i64, &str>(&column_name)?;
                            p.set_usize(Some(value as u32))?;
                        }
                        Primitive::U8(_) => {
                            let value = row.try_get::<i64, &str>(&column_name)?;
                            p.set_u8(Some(value as u8))?;
                        }
                        Primitive::U16(_) => {
                            let value = row.try_get::<i64, &str>(&column_name)?;
                            p.set_u16(Some(value as u16))?;
                        }
                        Primitive::U32(_) => {
                            let value = row.try_get::<i64, &str>(&column_name)?;
                            p.set_u32(Some(value as u32))?;
                        }
                        Primitive::U64(_) => {
                            let value = row.try_get::<i64, &str>(&column_name)?;
                            p.set_u64(Some(value as u64))?;
                        }
                        Primitive::U128(_) => {
                            let value = row.try_get::<String, &str>(&column_name)?;
                            let hex_str = value.trim_start_matches("0x");
                            p.set_u128(Some(
                                u128::from_str_radix(hex_str, 16)
                                    .map_err(ParseError::ParseIntError)?,
                            ))?;
                        }
                        Primitive::U256(_) => {
                            let value = row.try_get::<String, &str>(&column_name)?;
                            let hex_str = value.trim_start_matches("0x");
                            p.set_u256(Some(U256::from_be_hex(hex_str)))?;
                        }
                        Primitive::Felt252(_) => {
                            let value = row.try_get::<String, &str>(&column_name)?;
                            p.set_felt252(Some(
                                FieldElement::from_str(&value).map_err(ParseError::FromStr)?,
                            ))?;
                        }
                        Primitive::ClassHash(_) => {
                            let value = row.try_get::<String, &str>(&column_name)?;
                            p.set_class_hash(Some(
                                FieldElement::from_str(&value).map_err(ParseError::FromStr)?,
                            ))?;
                        }
                        Primitive::ContractAddress(_) => {
                            let value = row.try_get::<String, &str>(&column_name)?;
                            p.set_contract_address(Some(
                                FieldElement::from_str(&value).map_err(ParseError::FromStr)?,
                            ))?;
                        }
                    };
                }
                Ty::Enum(e) => {
                    let value = row.try_get::<String, &str>(&column_name)?;
                    e.set_option(&value)?;
                }
                Ty::Struct(nested) => {
                    let path = [path, &nested.name].join("$");
                    populate_struct_from_row(&path, nested, row)?;
                }
                ty => {
                    unimplemented!("unimplemented type_enum: {ty}");
                }
            };
        }

        Ok(())
    }

    rows.iter()
        .map(|row| {
            let mut struct_ty = schema.clone();
            populate_struct_from_row(&schema.name, &mut struct_ty, row)?;

            Ok(Ty::Struct(struct_ty))
        })
        .collect::<Result<Vec<Ty>, Error>>()
}

#[cfg(test)]
mod tests {
    use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};

    use super::{build_sql_model_query, SqlModelMember};
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
                id: "Position$Vec2".into(),
                name: "x".into(),
                r#type: "u256".into(),
                key: false,
                model_idx: 1,
                member_idx: 0,
                type_enum: "Primitive".into(),
                enum_options: None,
            },
            SqlModelMember {
                id: "Position$Vec2".into(),
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

        let query = build_sql_model_query(ty.as_struct().unwrap());
        assert_eq!(
            query,
            "SELECT Position.external_name AS \"Position.name\", Position.external_age AS \
             \"Position.age\", Position$Vec2.external_x AS \"Position$Vec2.x\", \
             Position$Vec2.external_y AS \"Position$Vec2.y\" FROM Position LEFT JOIN \
             Position$Vec2 ON Position.entity_id = Position$Vec2.entity_id ORDER BY \
             Position.event_id"
        );
    }
}
