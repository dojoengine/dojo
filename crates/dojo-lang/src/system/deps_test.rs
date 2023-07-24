use std::sync::Arc;

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_defs::db::DefsGroup;
use cairo_lang_defs::ids::ModuleItemId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::test_utils::setup_test_module;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_utils::{extract_matches, Upcast};
use dojo_test_utils::compiler::{build_test_config, build_test_db};
use indoc::indoc;
use scarb::ops;

#[test]
fn test_deps_extraction() {
    let mut db = build_test_db("src/manifest_test_crate/Scarb.toml").unwrap();
    let module_id = setup_test_module(
        &mut db,
        indoc! {"
            #[derive(Component, Copy, Drop, Serde, SerdeLen)]
            struct Moves {
                remaining: u8, 
            }

            #[derive(Component, Copy, Drop, Serde, SerdeLen)]
            struct Position {
                x: u32,
                y: u32
            }

            #[system]
            mod spawn {
                use array::ArrayTrait;
                use box::BoxTrait;
                use traits::Into;
                use dojo::world::Context;

                use super::Position;
                use super::Moves;

                fn execute(ctx: Context) {
                    set!(
                        ctx.world, ctx.origin.into(), (Moves { remaining: 10 }, Position { x: 0, y: 0 }, )
                    );
                    let value = get!(ctx.world, ctx.origin.into(), (Moves, Position, ));
                    return ();
                }
            }
        "},
    )
    .unwrap()
    .module_id;

    let system_id = extract_matches!(
        db.module_item_by_name(module_id, "spawn".into()).unwrap().unwrap(),
        ModuleItemId::Submodule
    );

    // system_id
    // .has_attr(db.upcast(), "system")?
    // let extractor = DepsExtractor::trait_as_interface_abi(db, system_id).unwrap();
    // let actual_serialization = serde_json::to_string_pretty(&abi).unwrap();
    // assert_eq!(
    //     actual_serialization,
    //     indoc! {
    //     r#"[
    //         {
    //           "type": "enum",
    //           "name": "core::option::Option::<()>",
    //           "variants": [
    //             {
    //               "name": "Some",
    //               "type": "()"
    //             },
    //             {
    //               "name": "None",
    //               "type": "()"
    //             }
    //           ]
    //         },
    //         {
    //           "type": "function",
    //           "name": "foo",
    //           "inputs": [
    //             {
    //               "name": "a",
    //               "type": "core::felt252"
    //             },
    //             {
    //               "name": "b",
    //               "type": "core::integer::u128"
    //             }
    //           ],
    //           "outputs": [
    //             {
    //               "type": "core::option::Option::<()>"
    //             }
    //           ],
    //           "state_mutability": "external"
    //         },
    //         {
    //           "type": "struct",
    //           "name": "core::integer::u256",
    //           "members": [
    //             {
    //               "name": "low",
    //               "type": "core::integer::u128"
    //             },
    //             {
    //               "name": "high",
    //               "type": "core::integer::u128"
    //             }
    //           ]
    //         },
    //         {
    //           "type": "struct",
    //           "name": "test::MyStruct::<core::integer::u256>",
    //           "members": [
    //             {
    //               "name": "a",
    //               "type": "core::integer::u256"
    //             },
    //             {
    //               "name": "b",
    //               "type": "core::felt252"
    //             }
    //           ]
    //         },
    //         {
    //           "type": "function",
    //           "name": "foo_external",
    //           "inputs": [
    //             {
    //               "name": "a",
    //               "type": "core::felt252"
    //             },
    //             {
    //               "name": "b",
    //               "type": "core::integer::u128"
    //             }
    //           ],
    //           "outputs": [
    //             {
    //               "type": "test::MyStruct::<core::integer::u256>"
    //             }
    //           ],
    //           "state_mutability": "external"
    //         },
    //         {
    //           "type": "enum",
    //           "name": "test::MyEnum::<core::integer::u128>",
    //           "variants": [
    //             {
    //               "name": "a",
    //               "type": "core::integer::u256"
    //             },
    //             {
    //               "name": "b",
    //               "type": "test::MyStruct::<core::integer::u128>"
    //             }
    //           ]
    //         },
    //         {
    //           "type": "function",
    //           "name": "foo_view",
    //           "inputs": [
    //             {
    //               "name": "a",
    //               "type": "core::felt252"
    //             },
    //             {
    //               "name": "b",
    //               "type": "core::integer::u128"
    //             }
    //           ],
    //           "outputs": [
    //             {
    //               "type": "test::MyEnum::<core::integer::u128>"
    //             }
    //           ],
    //           "state_mutability": "view"
    //         },
    //         {
    //           "type": "function",
    //           "name": "empty",
    //           "inputs": [],
    //           "outputs": [],
    //           "state_mutability": "external"
    //         }
    //       ]"#}
    // );
}
