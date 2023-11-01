use async_trait::async_trait;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use dojo_world::contracts::model::ModelReader;
use sqlx::{Pool, Sqlite};
use starknet::core::types::FieldElement;

use super::error::{self, Error};

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

        Ok(Self { name: name.clone(), class_hash, pool, packed_size, unpacked_size, layout })
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

#[cfg(test)]
mod tests {
    use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};

    use super::SqlModelMember;
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
}
