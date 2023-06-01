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
    let target_dir = Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap();

    let sequencer = Arc::new(RwLock::new(KatanaSequencer::new(StarknetConfig {
        total_accounts: 1,
        allow_zero_max_fee: true,
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
                    "0x06f62894bfd81d2e396ce266b2ad0f21e0668d604e5bb1077337b6d570a54aea",
                )
                .unwrap(),
            ),
            private_key: Some(
                FieldElement::from_hex_be(
                    "0x07230b49615d175307d580c33d6fda61fc7b9aec91df0f5c1a5ebe3b8cbfee02",
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
