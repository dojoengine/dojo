use dojo_types::schema::{Member, Struct, Ty};
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

#[derive(Debug, sqlx::FromRow)]
pub struct SqlModelMember {
    id: String,
    model_idx: u32,
    member_idx: u32,
    name: String,
    r#type: String,
    type_enum: SqlTypeEnum,
    key: bool,
}

#[derive(
    AsRefStr, Display, EnumIter, EnumString, Clone, Debug, Serialize, Deserialize, PartialEq,
)]
pub enum SqlTypeEnum {
    Primitive,
    Struct,
    Enum,
    Tuple,
}

// assume that the model members are sorted by model_idx and member_idx
// `id` is the type id of the model member
/// A helper function to parse the model members from sql table to `Ty`
pub fn parse_sql_model_members(path: &str, model_members_all: &[SqlModelMember]) -> Ty {
    let children = model_members_all
        .iter()
        .filter(|member| &member.id == &path)
        .map(|child| match child.type_enum {
            SqlTypeEnum::Primitive => Member {
                key: child.key,
                name: child.name.to_owned(),
                ty: Ty::Primitive(child.r#type.parse().unwrap()),
            },

            SqlTypeEnum::Struct => Member {
                key: child.key,
                name: child.name.to_owned(),
                ty: parse_sql_model_members(&child.id, model_members_all),
            },

            _ => todo!(),
        })
        .collect::<Vec<Member>>();

    // refer to the sql table for `model_members`
    let model_name = path.split("$").last().unwrap_or(path);

    Ty::Struct(Struct { name: model_name.to_owned(), children })
}

#[cfg(test)]
mod tests {
    use dojo_types::schema::{Member, Struct, Ty};

    use super::{SqlModelMember, SqlTypeEnum};
    use crate::server::utils::parse_sql_model_members;

    #[test]
    fn parse_simple_model_members_to_ty() {
        let model_members = vec![
            SqlModelMember {
                id: "Position".into(),
                name: "x".into(),
                r#type: "uint256".into(),
                key: false,
                model_idx: 0,
                member_idx: 0,
                type_enum: SqlTypeEnum::Primitive,
            },
            SqlModelMember {
                id: "Position".into(),
                name: "y".into(),
                r#type: "uint256".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: SqlTypeEnum::Primitive,
            },
        ];

        let expected_ty = Ty::Struct(Struct {
            name: "Position".into(),
            children: vec![
                dojo_types::schema::Member {
                    name: "x".into(),
                    key: false,
                    ty: Ty::Primitive("uint256".parse().unwrap()),
                },
                dojo_types::schema::Member {
                    name: "y".into(),
                    key: false,
                    ty: Ty::Primitive("uint256".parse().unwrap()),
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
                type_enum: SqlTypeEnum::Primitive,
            },
            SqlModelMember {
                id: "Position".into(),
                name: "age".into(),
                r#type: "u8".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: SqlTypeEnum::Primitive,
            },
            SqlModelMember {
                id: "Position".into(),
                name: "vec".into(),
                r#type: "Vec2".into(),
                key: false,
                model_idx: 0,
                member_idx: 1,
                type_enum: SqlTypeEnum::Struct,
            },
            SqlModelMember {
                id: "Position$Vec2".into(),
                name: "x".into(),
                r#type: "uint256".into(),
                key: false,
                model_idx: 1,
                member_idx: 0,
                type_enum: SqlTypeEnum::Primitive,
            },
            SqlModelMember {
                id: "Position$Vec2".into(),
                name: "y".into(),
                r#type: "uint256".into(),
                key: false,
                model_idx: 1,
                member_idx: 1,
                type_enum: SqlTypeEnum::Primitive,
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
                                ty: Ty::Primitive("uint256".parse().unwrap()),
                            },
                            Member {
                                name: "y".into(),
                                key: false,
                                ty: Ty::Primitive("uint256".parse().unwrap()),
                            },
                        ],
                    }),
                },
            ],
        });

        assert_eq!(parse_sql_model_members("Position", &model_members), expected_ty);
    }
}
