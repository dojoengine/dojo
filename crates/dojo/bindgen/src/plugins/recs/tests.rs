use std::collections::HashMap;
use std::path::Path;

use assert_matches::assert_matches;
use cainome::parser::tokens::{
    Composite, CompositeInner, CompositeInnerKind, CompositeType, Token,
};

use crate::plugins::recs::TypescriptRecsPlugin;
use crate::{BuiltinPlugin, DojoData, DojoWorld};

#[tokio::test]
async fn test_typescript_plugin_generate_code() {
    let plugin = TypescriptRecsPlugin::new();
    let data = create_mock_dojo_data();

    let result = plugin.generate_code(&data).await;

    assert_matches!(result, Ok(output) => {
        assert_eq!(output.len(), 2);
        assert!(output.contains_key(Path::new("models.gen.ts")));
        assert!(output.contains_key(Path::new("contracts.gen.ts")));

        // Check content of models.gen.ts
        let models_content = String::from_utf8_lossy(&output[Path::new("models.gen.ts")]);
        assert!(models_content.contains("import { defineComponent, Type as RecsType, World } from \"@dojoengine/recs\";"));
        assert!(models_content.contains("export type ContractComponents = Awaited<ReturnType<typeof defineContractComponents>>;"));

        // Check content of contracts.gen.ts
        let contracts_content = String::from_utf8_lossy(&output[Path::new("contracts.gen.ts")]);
        assert!(contracts_content.contains("import { Account, byteArray } from \"starknet\";"));
        assert!(contracts_content.contains("import { DojoProvider } from \"@dojoengine/core\";"));
        assert!(contracts_content.contains("export type IWorld = Awaited<ReturnType<typeof setupWorld>>;"));
    });
}

#[test]
fn test_map_type() {
    let bool_token =
        Token::CoreBasic(cainome::parser::tokens::CoreBasic { type_path: "bool".to_string() });
    assert_eq!(TypescriptRecsPlugin::map_type(&bool_token), "RecsType.Boolean");

    let u32_token =
        Token::CoreBasic(cainome::parser::tokens::CoreBasic { type_path: "u32".to_string() });
    assert_eq!(TypescriptRecsPlugin::map_type(&u32_token), "RecsType.Number");
}

#[test]
fn test_formatted_contract_name() {
    assert_eq!(TypescriptRecsPlugin::formatted_contract_name("dojo_examples-actions"), "actions");
    assert_eq!(TypescriptRecsPlugin::formatted_contract_name("my-contract"), "contract");
}

#[test]
fn test_get_namespace_from_tag() {
    assert_eq!(
        TypescriptRecsPlugin::get_namespace_from_tag("dojo_examples-actions"),
        "dojo_examples"
    );
    assert_eq!(TypescriptRecsPlugin::get_namespace_from_tag("my-contract"), "my");
}

#[test]
fn test_format_model() {
    // Create a mock Composite representing a model
    let model = Composite {
        type_path: "game::models::Position".to_string(),
        r#type: CompositeType::Struct,
        generic_args: vec![],
        inners: vec![
            CompositeInner {
                index: 0,
                name: "x".to_string(),
                kind: CompositeInnerKind::Data,
                token: Token::CoreBasic(cainome::parser::tokens::CoreBasic {
                    type_path: "u32".to_string(),
                }),
            },
            CompositeInner {
                index: 1,
                name: "y".to_string(),
                kind: CompositeInnerKind::Data,
                token: Token::CoreBasic(cainome::parser::tokens::CoreBasic {
                    type_path: "u32".to_string(),
                }),
            },
            CompositeInner {
                index: 2,
                name: "player".to_string(),
                kind: CompositeInnerKind::Data,
                token: Token::Composite(Composite {
                    type_path: "game::models::Player".to_string(),
                    r#type: CompositeType::Struct,
                    generic_args: vec![],
                    inners: vec![],
                    is_event: false,
                    alias: None,
                }),
            },
        ],
        is_event: false,
        alias: None,
    };

    let namespace = "game";
    let formatted = TypescriptRecsPlugin::format_model(namespace, &model);

    let expected = r#"
    // Model definition for `game::models::Position` model
    Position: (() => {
        return defineComponent(
            world,
            {
                x: RecsType.Number,
                y: RecsType.Number,
                player: PlayerDefinition,
            },
            {
                metadata: {
                    namespace: "game",
                    name: "Position",
                    types: ["u32", "u32"],
                    customTypes: ["Player"],
                },
            }
        );
    })(),
"#;

    assert_eq!(formatted.replace([' ', '\n'], "").trim(), expected.replace([' ', '\n'], "").trim());
}

#[test]
fn test_format_enum_model() {
    // Create a mock Composite representing an enum model
    let model = Composite {
        type_path: "game::models::Direction".to_string(),
        r#type: CompositeType::Enum,
        generic_args: vec![],
        inners: vec![CompositeInner {
            index: 0,
            name: "North".to_string(),
            kind: CompositeInnerKind::Data,
            token: Token::CoreBasic(cainome::parser::tokens::CoreBasic {
                type_path: "()".to_string(),
            }),
        }],
        is_event: false,
        alias: None,
    };

    let namespace = "game";
    let formatted = TypescriptRecsPlugin::format_model(namespace, &model);

    let expected = r#"
    // Model definition for `game::models::Direction` model
    Direction: (() => {
        return defineComponent(
            world,
            {
                North: ()Definition,
            },
            {
                metadata: {
                    namespace: "game",
                    name: "Direction",
                    types: [],
                    customTypes: ["()"],
                },
            }
        );
    })(),
"#;

    // Remove all spaces and compare
    assert_eq!(formatted.replace([' ', '\n'], "").trim(), expected.replace([' ', '\n'], "").trim());
}

// Helper function to create mock DojoData for testing
fn create_mock_dojo_data() -> DojoData {
    DojoData {
        world: DojoWorld { name: 0x01.to_string() },
        models: HashMap::new(),
        contracts: HashMap::new(),
        events: HashMap::new(),
        other_types: HashMap::new(),
    }
}
