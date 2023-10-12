use dojo_test_utils::rpc::MockJsonRpcTransport;
use serde_json::json;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcMethod};

use super::Manifest;
use crate::manifest::ManifestError;

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
    let err = Manifest::load_from_remote(rpc, FieldElement::ONE, None).await.unwrap_err();

    match err {
        ManifestError::RemoteWorldNotFound => {
            // World not deployed.
        }
        err => panic!("Unexpected error: {err}"),
    }
}
