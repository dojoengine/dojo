use camino::Utf8PathBuf;
use dojo_test_utils::rpc::MockJsonRpcTransport;
use dojo_test_utils::sequencer::Sequencer;
use serde_json::json;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcMethod};

use super::Manifest;
use crate::config::{EnvironmentConfig, WorldConfig};
use crate::manifest::ManifestError;
use crate::migration::strategy::prepare_for_migration;
use crate::migration::world::WorldDiff;

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
    let err = Manifest::from_remote(rpc, FieldElement::ONE, None).await.unwrap_err();

    match err {
        ManifestError::WorldNotFound => {
            // World not deployed.
        }
        err => panic!("Unexpected error: {err}"),
    }
}

#[tokio::test]
async fn test_migration_from_remote() {
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = Sequencer::start().await;
    let account = sequencer.account();
    let world_config = WorldConfig::default();
    let env_config = EnvironmentConfig {
        rpc: Some(sequencer.url()),
        account_address: Some(account.address),
        private_key: Some(account.private_key),
        ..EnvironmentConfig::default()
    };

    let migrator = env_config.migrator().await.unwrap();
    let diff = WorldDiff::from_path(target_dir.clone(), &world_config, &env_config).await.unwrap();
    let mut migration = prepare_for_migration(target_dir.clone(), diff, world_config).unwrap();
    let migration_result = migration.execute(migrator).await.unwrap();

    let _local_manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let _remote_manifest = Manifest::from_remote(
        env_config.provider().unwrap(),
        migration_result.world.unwrap().contract_address,
        None,
    )
    .await
    .unwrap();

    sequencer.stop().unwrap();
}
