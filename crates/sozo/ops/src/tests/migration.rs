#![allow(dead_code)]
use std::sync::Arc;

use anyhow::Result;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::copy_spawn_and_move_db;
use dojo_utils::TxnConfig;
use dojo_world::contracts::WorldContract;
use dojo_world::diff::WorldDiff;
use katana_runner::RunnerCtx;
use scarb::compiler::Profile;
use sozo_scarbext::WorkspaceExt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;

use crate::migrate::{Migration, MigrationResult};
use crate::migration_ui::MigrationUi;

/// Sets up the world diff from the environment and returns the world diff used to create a
/// migration.
async fn setup_migration(
    example_project: &str,
    profile: Profile,
    provider: Arc<JsonRpcClient<HttpTransport>>,
) -> Result<WorldDiff> {
    let setup = CompilerTestSetup::from_examples("../../dojo/core", "../../../examples/");
    let config = setup.build_test_config(example_project, profile);

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let world_local = ws.load_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let world_diff = WorldDiff::new_from_chain(world_address, world_local, &provider, None).await?;

    Ok(world_diff)
}

/// Migrates the spawn-and-move project from the local environment.
async fn migrate_spawn_and_move(sequencer: &RunnerCtx) -> MigrationResult {
    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_diff = setup_migration("spawn-and-move", Profile::DEV, provider)
        .await
        .expect("Failed to setup migration");

    let world_address = world_diff.world_info.address;
    let profile_config = world_diff.profile_config.clone();

    let migration = Migration::new(
        world_diff,
        WorldContract::new(world_address, &account),
        TxnConfig::init_wait(),
        profile_config,
        sequencer.url().to_string(),
    );

    let mut ui = MigrationUi::new(None).with_silent();

    migration.migrate(&mut ui).await.expect("Migration spawn-and-move failed.")
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10)]
async fn migrate_from_local(sequencer: &RunnerCtx) {
    let MigrationResult { manifest, has_changes } = migrate_spawn_and_move(sequencer).await;

    assert!(has_changes);
    assert_eq!(manifest.contracts.len(), 4);
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn migrate_no_change(sequencer: &RunnerCtx) {
    let MigrationResult { manifest, has_changes } = migrate_spawn_and_move(sequencer).await;
    assert!(!has_changes);
    assert_eq!(manifest.contracts.len(), 4);
}
