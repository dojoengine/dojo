use std::str::FromStr;

use indoc::indoc;
use starknet::core::types::FieldElement;

use crate::ProjectConfigContent;

#[test]
fn test_serde() {
    let config = ProjectConfigContent {
        crate_roots: [("crate".into(), "dir".into())].into_iter().collect(),
        world: crate::WorldConfig {
            address: Some(FieldElement::from_str("0xdead").unwrap()),
            initializer_class_hash: Some(FieldElement::from_str("0xbeef").unwrap()),
        },
        deployments: Some(crate::Deployments {
            testnet: Some(crate::Deployment {
                rpc: Some("https://starknet-goerli.rpc.gg/rpc/v0.2".into()),
            }),
            mainnet: Some(crate::Deployment {
                rpc: Some("https://starknet.rpc.gg/rpc/v0.2".into()),
            }),
        }),
    };
    let serialized = toml::to_string(&config).unwrap();
    // NOTE: FieldElement encodes back to bigint string
    assert_eq!(
        serialized,
        indoc! { r#"
            [crate_roots]
            crate = "dir"

            [world]
            address = "57005"
            initializer_class_hash = "48879"
            [deployments.testnet]
            rpc = "https://starknet-goerli.rpc.gg/rpc/v0.2"

            [deployments.mainnet]
            rpc = "https://starknet.rpc.gg/rpc/v0.2"
        "# }
    );
    assert_eq!(config, toml::from_str(&serialized).unwrap());
}
