use std::sync::Arc;

use camino::Utf8PathBuf;
use katana_core::sequencer::KatanaSequencer;
use katana_core::starknet::StarknetConfig;
use katana_rpc::config::RpcConfig;
use katana_rpc::KatanaNodeRpc;
use starknet::core::types::FieldElement;
use tokio::sync::RwLock;
use url::Url;

use crate::migration::world::World;
use crate::{EnvironmentConfig, WorldConfig};

#[tokio::test]
async fn test_migration() {
    let target_dir = Utf8PathBuf::from_path_buf("src/cairo_level_tests/target/dev".into()).unwrap();

    let sequencer = Arc::new(RwLock::new(KatanaSequencer::new(StarknetConfig {
        total_accounts: 1,
        ..StarknetConfig::default()
    })));
    sequencer.write().await.start();
    let (socket_addr, server_handle) =
        KatanaNodeRpc::new(sequencer.clone(), RpcConfig { port: 0 }).run().await.unwrap();
    let url = Url::parse(&format!("http://{}", socket_addr)).expect("Failed to parse URL");
    let world = World::from_path(
        target_dir.clone(),
        WorldConfig::default(),
        EnvironmentConfig {
            rpc: Some(url),
            account_address: Some(
                FieldElement::from_hex_be(
                    "0x0002dd34561535562f1e4befd4e5a3214772554d15e44e2493ab1695e1f83dc4",
                )
                .unwrap(),
            ),
            private_key: Some(
                FieldElement::from_hex_be(
                    "0x0000001800000000300000180000000000030000000000003006001800006600",
                )
                .unwrap(),
            ),
            ..EnvironmentConfig::default()
        },
    )
    .await
    .unwrap();

    let mut migration = world.prepare_for_migration(target_dir).await.unwrap();
    migration.execute().await.unwrap();

    server_handle.stop().unwrap();
}
