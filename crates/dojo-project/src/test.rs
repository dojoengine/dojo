use std::str::FromStr;

use indoc::indoc;
use starknet::core::types::FieldElement;

use crate::ProjectConfigContent;

#[test]
fn test_serde() {
    let config = ProjectConfigContent {
        crate_roots: [("crate".into(), "dir".into())].into_iter().collect(),
        world: crate::WorldConfig {
            name: "dojo".into(),
            address: FieldElement::from_str("0xdead").unwrap(),
        },
    };
    let serialized = toml::to_string(&config).unwrap();
    // NOTE: FieldElement encodes back to bigint string
    assert_eq!(
        serialized,
        indoc! { r#"
            [crate_roots]
            crate = "dir"

            [world]
            name = "dojo"
            address = "57005"
        "# }
    );
    assert_eq!(config, toml::from_str(&serialized).unwrap());
}
