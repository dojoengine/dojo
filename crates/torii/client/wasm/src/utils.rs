use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
use serde_json::Value;

pub fn parse_ty_as_json_str(ty: &Ty) -> String {
    fn parse_ty_as_json_str_impl(ty: &Ty) -> Value {
        match ty {
            Ty::Primitive(primitive) => primitive_value_json(*primitive),

            Ty::Struct(struct_ty) => struct_ty
                .children
                .iter()
                .map(|child| (child.name.to_owned(), parse_ty_as_json_str_impl(&child.ty)))
                .collect::<serde_json::Map<String, Value>>()
                .into(),

            Ty::Enum(enum_ty) => {
                if let Some(option) = enum_ty.option {
                    let option = &enum_ty.options[option as usize];
                    Value::String(option.name.to_owned())
                } else {
                    Value::Null
                }
            }

            Ty::Tuple(_) => unimplemented!("tuple not supported"),
        }
    }

    parse_ty_as_json_str_impl(ty).to_string()
}

fn primitive_value_json(primitive: Primitive) -> Value {
    match primitive {
        Primitive::Bool(Some(value)) => Value::Bool(value),
        Primitive::U8(Some(value)) => Value::Number(value.into()),
        Primitive::U16(Some(value)) => Value::Number(value.into()),
        Primitive::U32(Some(value)) => Value::Number(value.into()),
        Primitive::U64(Some(value)) => Value::Number(value.into()),
        Primitive::USize(Some(value)) => Value::Number(value.into()),
        Primitive::U128(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::U256(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::Felt252(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::ClassHash(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::ContractAddress(Some(value)) => Value::String(format!("{value:#x}")),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod test {

    use dojo_types::schema::{Enum, EnumOption, Member, Struct};
    use serde_json::json;
    use starknet::macros::felt;
    use wasm_bindgen_test::*;

    use super::*;

    #[wasm_bindgen_test]
    fn parse_ty_with_key() {
        let expected_ty = Ty::Struct(Struct {
            name: "Position".into(),
            children: vec![
                Member {
                    name: "game_id".into(),
                    key: true,
                    ty: Ty::Primitive(Primitive::Felt252(Some(felt!("0xdead")))),
                },
                Member {
                    name: "player".into(),
                    key: true,
                    ty: Ty::Primitive(Primitive::ContractAddress(Some(felt!("0xbeef")))),
                },
                Member {
                    name: "points".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U32(Some(200))),
                },
                Member {
                    name: "kind".into(),
                    key: false,
                    ty: Ty::Enum(Enum {
                        name: "PlayerKind".into(),
                        option: Some(1),
                        options: vec![
                            EnumOption { name: "Good".into(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Bad".into(), ty: Ty::Tuple(vec![]) },
                        ],
                    }),
                },
                Member {
                    name: "vec".into(),
                    key: false,
                    ty: Ty::Struct(Struct {
                        name: "vec".into(),
                        children: vec![
                            Member {
                                name: "x".into(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U128(Some(10))),
                            },
                            Member {
                                name: "y".into(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U128(Some(10))),
                            },
                        ],
                    }),
                },
            ],
        });

        let expected_json = json!({
            "points": 200,
            "kind": "Bad",
            "vec": {
                "x": "0xa",
                "y": "0xa",
            },
        });

        let actual_json = parse_ty_as_json_str(&expected_ty);
        assert_eq!(expected_json.to_string(), actual_json)
    }

    #[wasm_bindgen_test]
    fn parse_ty_to_value() {
        let expected_ty = Ty::Struct(Struct {
            name: "Position".into(),
            children: vec![
                Member {
                    name: "is_dead".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::Bool(Some(true))),
                },
                Member {
                    name: "points".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U32(Some(200))),
                },
                Member {
                    name: "kind".into(),
                    key: false,
                    ty: Ty::Enum(Enum {
                        name: "PlayerKind".into(),
                        option: Some(1),
                        options: vec![
                            EnumOption { name: "Good".into(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Bad".into(), ty: Ty::Tuple(vec![]) },
                        ],
                    }),
                },
                Member {
                    name: "vec".into(),
                    key: false,
                    ty: Ty::Struct(Struct {
                        name: "vec".into(),
                        children: vec![
                            Member {
                                name: "x".into(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U128(Some(10))),
                            },
                            Member {
                                name: "y".into(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U128(Some(10))),
                            },
                        ],
                    }),
                },
            ],
        });

        let expected_json = json!({
            "is_dead": true,
            "points": 200,
            "kind": "Bad",
            "vec": {
                "x": "0xa",
                "y": "0xa",
            },
        });

        let actual_json = parse_ty_as_json_str(&expected_ty);
        assert_eq!(expected_json.to_string(), actual_json)
    }
}
