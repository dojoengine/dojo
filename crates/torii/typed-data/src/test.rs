use crypto_bigint::U256;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use serde_json::Number;
use starknet_crypto::Felt;

use crate::typed_data::{map_ty_to_primitive, parse_value_to_ty, Domain, PrimitiveType, TypedData};

#[test]
fn test_parse_primitive_to_ty() {
    // primitives
    let mut ty = Ty::Primitive(Primitive::U8(None));
    let value = PrimitiveType::Number(Number::from(1u64));
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::U8(Some(1))));

    let mut ty = Ty::Primitive(Primitive::U16(None));
    let value = PrimitiveType::Number(Number::from(1u64));
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::U16(Some(1))));

    let mut ty = Ty::Primitive(Primitive::U32(None));
    let value = PrimitiveType::Number(Number::from(1u64));
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::U32(Some(1))));

    let mut ty = Ty::Primitive(Primitive::U64(None));
    let value = PrimitiveType::Number(Number::from(1u64));
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::U64(Some(1))));

    let mut ty = Ty::Primitive(Primitive::U128(None));
    let value = PrimitiveType::String("1".to_string());
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::U128(Some(1))));

    // test u256 with low high
    let mut ty = Ty::Primitive(Primitive::U256(None));
    let value = PrimitiveType::Object(
        vec![
            ("low".to_string(), PrimitiveType::String("1".to_string())),
            ("high".to_string(), PrimitiveType::String("0".to_string())),
        ]
        .into_iter()
        .collect(),
    );
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::U256(Some(U256::ONE))));

    let mut ty = Ty::Primitive(Primitive::Felt252(None));
    let value = PrimitiveType::String("1".to_string());
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::Felt252(Some(Felt::ONE))));

    let mut ty = Ty::Primitive(Primitive::ClassHash(None));
    let value = PrimitiveType::String("1".to_string());
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::ClassHash(Some(Felt::ONE))));

    let mut ty = Ty::Primitive(Primitive::ContractAddress(None));
    let value = PrimitiveType::String("1".to_string());
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))));

    let mut ty = Ty::Primitive(Primitive::EthAddress(None));
    let value = PrimitiveType::String("1".to_string());
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::EthAddress(Some(Felt::ONE))));

    let mut ty = Ty::Primitive(Primitive::Bool(None));
    let value = PrimitiveType::Bool(true);
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::Primitive(Primitive::Bool(Some(true))));

    // bytearray
    let mut ty = Ty::ByteArray("".to_string());
    let value = PrimitiveType::String("mimi".to_string());
    parse_value_to_ty(&value, &mut ty).unwrap();
    assert_eq!(ty, Ty::ByteArray("mimi".to_string()));
}

#[test]
fn test_map_ty_to_primitive() {
    let ty = Ty::Primitive(Primitive::U8(Some(1)));
    let value = PrimitiveType::Number(Number::from(1u64));
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::U16(Some(1)));
    let value = PrimitiveType::Number(Number::from(1u64));
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::U32(Some(1)));
    let value = PrimitiveType::Number(Number::from(1u64));
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::U64(Some(1)));
    let value = PrimitiveType::String("1".to_string());
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::U128(Some(1)));
    let value = PrimitiveType::String("1".to_string());
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::U256(Some(U256::ONE)));
    let value = PrimitiveType::Object(
        vec![
            ("low".to_string(), PrimitiveType::String("1".to_string())),
            ("high".to_string(), PrimitiveType::String("0".to_string())),
        ]
        .into_iter()
        .collect(),
    );
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::Felt252(Some(Felt::ONE)));
    let value = PrimitiveType::String("1".to_string());
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::ClassHash(Some(Felt::ONE)));
    let value = PrimitiveType::String("1".to_string());
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE)));
    let value = PrimitiveType::String("1".to_string());
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::EthAddress(Some(Felt::ONE)));
    let value = PrimitiveType::String("1".to_string());
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::Primitive(Primitive::Bool(Some(true)));
    let value = PrimitiveType::Bool(true);
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());

    let ty = Ty::ByteArray("mimi".to_string());
    let value = PrimitiveType::String("mimi".to_string());
    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());
}

#[test]
fn test_parse_complex_to_ty() {
    let mut ty = Ty::Struct(Struct {
        name: "PlayerConfig".to_string(),
        children: vec![
            Member {
                name: "player".to_string(),
                ty: Ty::Primitive(Primitive::ContractAddress(None)),
                key: true,
            },
            Member { name: "name".to_string(), ty: Ty::ByteArray("".to_string()), key: false },
            Member {
                name: "items".to_string(),
                // array of PlayerItem struct
                ty: Ty::Array(vec![Ty::Struct(Struct {
                    name: "PlayerItem".to_string(),
                    children: vec![
                        Member {
                            name: "item_id".to_string(),
                            ty: Ty::Primitive(Primitive::U32(None)),
                            key: false,
                        },
                        Member {
                            name: "quantity".to_string(),
                            ty: Ty::Primitive(Primitive::U32(None)),
                            key: false,
                        },
                    ],
                })]),
                key: false,
            },
            // a favorite_item field with enum type Option<PlayerItem>
            Member {
                name: "favorite_item".to_string(),
                ty: Ty::Enum(Enum {
                    name: "Option".to_string(),
                    option: None,
                    options: vec![
                        EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                        EnumOption {
                            name: "Some".to_string(),
                            ty: Ty::Struct(Struct {
                                name: "PlayerItem".to_string(),
                                children: vec![
                                    Member {
                                        name: "item_id".to_string(),
                                        ty: Ty::Primitive(Primitive::U32(None)),
                                        key: false,
                                    },
                                    Member {
                                        name: "quantity".to_string(),
                                        ty: Ty::Primitive(Primitive::U32(None)),
                                        key: false,
                                    },
                                ],
                            }),
                        },
                    ],
                }),
                key: false,
            },
        ],
    });

    let value = PrimitiveType::Object(
        vec![
            ("player".to_string(), PrimitiveType::String("1".to_string())),
            ("name".to_string(), PrimitiveType::String("mimi".to_string())),
            (
                "items".to_string(),
                PrimitiveType::Array(vec![PrimitiveType::Object(
                    vec![
                        ("item_id".to_string(), PrimitiveType::String("1".to_string())),
                        ("quantity".to_string(), PrimitiveType::Number(Number::from(1u64))),
                    ]
                    .into_iter()
                    .collect(),
                )]),
            ),
            (
                "favorite_item".to_string(),
                PrimitiveType::Object(
                    vec![(
                        "Some".to_string(),
                        PrimitiveType::Object(
                            vec![
                                ("item_id".to_string(), PrimitiveType::String("1".to_string())),
                                ("quantity".to_string(), PrimitiveType::Number(Number::from(1u64))),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                    )]
                    .into_iter()
                    .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );

    parse_value_to_ty(&value, &mut ty).unwrap();

    assert_eq!(
        ty,
        Ty::Struct(Struct {
            name: "PlayerConfig".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))),
                    key: true,
                },
                Member {
                    name: "name".to_string(),
                    ty: Ty::ByteArray("mimi".to_string()),
                    key: false,
                },
                Member {
                    name: "items".to_string(),
                    ty: Ty::Array(vec![Ty::Struct(Struct {
                        name: "PlayerItem".to_string(),
                        children: vec![
                            Member {
                                name: "item_id".to_string(),
                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                key: false,
                            },
                            Member {
                                name: "quantity".to_string(),
                                ty: Ty::Primitive(Primitive::U32(Some(1))),
                                key: false,
                            },
                        ],
                    })]),
                    key: false,
                },
                Member {
                    name: "favorite_item".to_string(),
                    ty: Ty::Enum(Enum {
                        name: "Option".to_string(),
                        option: Some(1_u8),
                        options: vec![
                            EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption {
                                name: "Some".to_string(),
                                ty: Ty::Struct(Struct {
                                    name: "PlayerItem".to_string(),
                                    children: vec![
                                        Member {
                                            name: "item_id".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                                            key: false,
                                        },
                                        Member {
                                            name: "quantity".to_string(),
                                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                                            key: false,
                                        },
                                    ],
                                }),
                            },
                        ]
                    }),
                    key: false,
                },
            ],
        })
    );
}

#[test]
fn test_map_ty_to_complex() {
    let ty = Ty::Struct(Struct {
        name: "PlayerConfig".to_string(),
        children: vec![
            Member {
                name: "player".to_string(),
                ty: Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))),
                key: true,
            },
            Member { name: "name".to_string(), ty: Ty::ByteArray("mimi".to_string()), key: false },
            Member {
                name: "items".to_string(),
                ty: Ty::Array(vec![Ty::Struct(Struct {
                    name: "PlayerItem".to_string(),
                    children: vec![
                        Member {
                            name: "item_id".to_string(),
                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                            key: false,
                        },
                        Member {
                            name: "quantity".to_string(),
                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                            key: false,
                        },
                    ],
                })]),
                key: false,
            },
            Member {
                name: "favorite_item".to_string(),
                ty: Ty::Enum(Enum {
                    name: "Option".to_string(),
                    option: Some(1_u8),
                    options: vec![
                        EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                        EnumOption {
                            name: "Some".to_string(),
                            ty: Ty::Struct(Struct {
                                name: "PlayerItem".to_string(),
                                children: vec![
                                    Member {
                                        name: "item_id".to_string(),
                                        ty: Ty::Primitive(Primitive::U32(Some(1))),
                                        key: false,
                                    },
                                    Member {
                                        name: "quantity".to_string(),
                                        ty: Ty::Primitive(Primitive::U32(Some(1))),
                                        key: false,
                                    },
                                ],
                            }),
                        },
                    ],
                }),
                key: false,
            },
        ],
    });

    let value = PrimitiveType::Object(
        vec![
            ("player".to_string(), PrimitiveType::String("1".to_string())),
            ("name".to_string(), PrimitiveType::String("mimi".to_string())),
            (
                "items".to_string(),
                PrimitiveType::Array(vec![PrimitiveType::Object(
                    vec![
                        ("item_id".to_string(), PrimitiveType::Number(Number::from(1u64))),
                        ("quantity".to_string(), PrimitiveType::Number(Number::from(1u64))),
                    ]
                    .into_iter()
                    .collect(),
                )]),
            ),
            (
                "favorite_item".to_string(),
                PrimitiveType::Object(
                    vec![(
                        "Some".to_string(),
                        PrimitiveType::Object(
                            vec![
                                ("item_id".to_string(), PrimitiveType::Number(Number::from(1u64))),
                                ("quantity".to_string(), PrimitiveType::Number(Number::from(1u64))),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                    )]
                    .into_iter()
                    .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );

    assert_eq!(value, map_ty_to_primitive(&ty).unwrap());
}

#[test]
fn test_model_to_typed_data() {
    let ty = Ty::Struct(Struct {
        name: "PlayerConfig".to_string(),
        children: vec![
            Member {
                name: "player".to_string(),
                ty: Ty::Primitive(Primitive::ContractAddress(Some(Felt::ONE))),
                key: true,
            },
            Member { name: "name".to_string(), ty: Ty::ByteArray("mimi".to_string()), key: false },
            Member {
                name: "items".to_string(),
                // array of PlayerItem struct
                ty: Ty::Array(vec![Ty::Struct(Struct {
                    name: "PlayerItem".to_string(),
                    children: vec![
                        Member {
                            name: "item_id".to_string(),
                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                            key: false,
                        },
                        Member {
                            name: "quantity".to_string(),
                            ty: Ty::Primitive(Primitive::U32(Some(1))),
                            key: false,
                        },
                    ],
                })]),
                key: false,
            },
            // a favorite_item field with enum type Option<PlayerItem>
            Member {
                name: "favorite_item".to_string(),
                ty: Ty::Enum(Enum {
                    name: "Option".to_string(),
                    option: Some(1),
                    options: vec![
                        EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                        EnumOption {
                            name: "Some".to_string(),
                            ty: Ty::Struct(Struct {
                                name: "PlayerItem".to_string(),
                                children: vec![
                                    Member {
                                        name: "item_id".to_string(),
                                        ty: Ty::Primitive(Primitive::U32(Some(69))),
                                        key: false,
                                    },
                                    Member {
                                        name: "quantity".to_string(),
                                        ty: Ty::Primitive(Primitive::U32(Some(42))),
                                        key: false,
                                    },
                                ],
                            }),
                        },
                    ],
                }),
                key: false,
            },
        ],
    });

    let typed_data =
        TypedData::from_model(ty, Domain::new("Test", "1", "Test", Some("1"))).unwrap();

    let path = "mocks/model_PlayerConfig.json";
    let file = std::fs::File::open(path).unwrap();
    let reader = std::io::BufReader::new(file);

    let file_typed_data: TypedData = serde_json::from_reader(reader).unwrap();

    assert_eq!(typed_data.encode(Felt::ZERO).unwrap(), file_typed_data.encode(Felt::ZERO).unwrap());
}
