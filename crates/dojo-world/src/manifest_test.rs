use serde_json::json;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcMethod};

use super::Manifest;
use crate::manifest::{ManifestError, EXECUTOR_ADDRESS_SLOT};
use crate::test_utils::MockJsonRpcTransport;

#[tokio::test]
async fn test_manifest_from_remote_throw_error_on_not_deployed() {
    let mut mock_transport = MockJsonRpcTransport::new();
    mock_transport.set_response(
        JsonRpcMethod::GetClassHashAt,
        json!(["pending", "0x1"]),
        json!({
            "id": 1,
            "error": {
                "code": 20,
                "message": "Contract not found"
            },
        }),
    );

    let rpc = JsonRpcClient::new(mock_transport);
    let err = Manifest::from_remote(FieldElement::ONE, rpc, None).await.unwrap_err();

    match err {
        ManifestError::NotDeployed => {
            // World not deployed.
        }
        err => panic!("Unexpected error: {err}"),
    }
}

#[tokio::test]
async fn test_manifest_loads_empty_world_from_remote() {
    let world_address = FieldElement::ONE;
    let world_class_hash = FieldElement::TWO;
    let executor_address = FieldElement::THREE;
    let executor_class_hash = FieldElement::from_hex_be("0x4").unwrap();

    let mut mock_transport = MockJsonRpcTransport::new();
    mock_transport.set_response(
        JsonRpcMethod::GetClassHashAt,
        json!(["pending", format!("{world_address:#x}")]),
        json!({
            "id": 1,
            "result": format!("{world_class_hash:#x}")
        }),
    );

    mock_transport.set_response(
        JsonRpcMethod::GetStorageAt,
        json!(["0x1", format!("{EXECUTOR_ADDRESS_SLOT:#x}"), "pending"]),
        json!({
            "id": 1,
            "result": format!("{executor_address:#x}")
        }),
    );

    mock_transport.set_response(
        JsonRpcMethod::GetClassHashAt,
        json!(["pending", format!("{executor_address:#x}")]),
        json!({
            "id": 1,
            "result": format!("{executor_class_hash:#x}")
        }),
    );

    let rpc = JsonRpcClient::new(mock_transport);
    let manifest = Manifest::from_remote(FieldElement::ONE, rpc, None).await.unwrap();

    assert_eq!(
        manifest,
        Manifest {
            world: Some(world_class_hash),
            executor: Some(executor_class_hash),
            ..Manifest::default()
        }
    )
}
