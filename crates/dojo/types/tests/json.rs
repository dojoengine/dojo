use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use serde_json::json;

#[test]
fn serialize_ty_to_json() {
    let ty = Ty::Struct(Struct {
        name: "Position".into(),
        children: vec![
            Member { name: "x".into(), key: false, ty: Ty::Primitive(Primitive::U8(Some(128))) },
            Member { name: "y".into(), key: false, ty: Ty::Primitive(Primitive::U64(Some(2048))) },
            Member {
                name: "kind".into(),
                key: false,
                ty: Ty::Enum(Enum {
                    name: "PositionKind".into(),
                    option: Some(1),
                    options: vec![
                        EnumOption { name: "Kind1".into(), ty: Ty::Tuple(vec![]) },
                        EnumOption { name: "Kind2".into(), ty: Ty::Tuple(vec![]) },
                    ],
                }),
            },
        ],
    });

    let actual_value = serde_json::to_value(ty).unwrap();
    let expected_value = json!({
        "type": "struct",
        "content": {
            "name": "Position",
            "children": [
                {
                    "name": "x",
                    "member_type": {
                        "type": "primitive",
                        "content": {
                            "scalar_type": "u8",
                            "value": 128
                        }
                    },
                    "key": false
                },
                {
                    "name": "y",
                    "member_type": {
                        "type": "primitive",
                        "content": {
                            "scalar_type": "u64",
                            "value": 2048
                        }
                    },
                    "key": false
                },
                {
                    "name": "kind",
                    "member_type": {
                        "type": "enum",
                        "content": {
                          "name": "PositionKind",
                          "option": 1,
                          "options": [
                            {
                                "name": "Kind1",
                                "ty": {
                                    "type": "tuple",
                                    "content": []
                                }
                            },
                            {
                                "name": "Kind2",
                                "ty": {
                                    "type": "tuple",
                                    "content": []
                                }
                            },
                          ]
                        }
                    },
                    "key": false
                }
            ]
        }
    });

    assert_eq!(actual_value, expected_value)
}

#[test]
fn deserialize_ty_from_json() {
    let json = json!({
        "type": "struct",
        "content": {
            "name": "Position",
            "children": [
                {
                    "name": "x",
                    "member_type": {
                        "type": "primitive",
                        "content": {
                            "scalar_type": "u8",
                            "value": 128
                        }
                    },
                    "key": false
                },
                {
                    "name": "y",
                    "member_type": {
                        "type": "primitive",
                        "content": {
                            "scalar_type": "u64",
                            "value": 2048
                        }
                    },
                    "key": false
                },
                {
                    "name": "kind",
                    "member_type": {
                        "type": "enum",
                        "content": {
                          "name": "PositionKind",
                          "option": 1,
                          "options": [
                             {
                                "name": "Kind1",
                                "ty": {
                                    "type": "tuple",
                                    "content": []
                                }
                            },
                            {
                                "name": "Kind2",
                                "ty": {
                                    "type": "tuple",
                                    "content": []
                                }
                            },
                          ]
                        }
                    },
                    "key": false
                }
            ]
        }
    });

    let expected_value = Ty::Struct(Struct {
        name: "Position".into(),
        children: vec![
            Member { name: "x".into(), key: false, ty: Ty::Primitive(Primitive::U8(Some(128))) },
            Member { name: "y".into(), key: false, ty: Ty::Primitive(Primitive::U64(Some(2048))) },
            Member {
                name: "kind".into(),
                key: false,
                ty: Ty::Enum(Enum {
                    name: "PositionKind".into(),
                    option: Some(1),
                    options: vec![
                        EnumOption { name: "Kind1".into(), ty: Ty::Tuple(vec![]) },
                        EnumOption { name: "Kind2".into(), ty: Ty::Tuple(vec![]) },
                    ],
                }),
            },
        ],
    });

    let actual_value: Ty = serde_json::from_value(json).unwrap();
    assert_eq!(actual_value, expected_value)
}
