use std::collections::HashMap;
use std::path::Path;

use super::{BuiltinPlugin, TypescriptScaffoldPlugin};
use crate::{DojoData, DojoWorld};

#[tokio::test]
async fn test_generate_code_contains_all_keys() {
    let plugin = TypescriptScaffoldPlugin::new();
    let data = create_empty_mock_dojo_data();

    let result = plugin.generate_code(&data).await;

    assert!(result.is_ok());

    let res = result.expect("failed to generate code");
    assert_eq!(res.keys().len(), 5);
}

#[tokio::test]
async fn test_handle_system() {
    let plugin = TypescriptScaffoldPlugin::new();
    let data = create_empty_mock_dojo_data();

    let result = plugin.generate_code(&data).await;

    assert!(result.is_ok());

    let res = result.expect("failed to generate code");
    let _system =
        std::str::from_utf8(res.get(Path::new("system.ts")).expect("failed to get system"))
            .expect("system should be valid utf8");
}

/// Creates an empty dojo mock
fn create_empty_mock_dojo_data() -> DojoData {
    DojoData {
        world: DojoWorld { name: 0x01.to_string() },
        models: HashMap::new(),
        contracts: HashMap::new(),
    }
}
