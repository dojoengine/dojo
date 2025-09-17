#![allow(dead_code)]
use std::sync::Arc;

use anyhow::Result;
use dojo_test_utils::migration::copy_spawn_and_move_db;
use dojo_test_utils::setup::TestSetup;
use dojo_utils::TxnConfig;
use dojo_world::config::ResourceConfig;
use dojo_world::contracts::WorldContract;
use dojo_world::diff::WorldDiff;
use dojo_world::services::MockUploadService;
use katana_runner::RunnerCtx;
use scarb_interop::Profile;
use scarb_metadata_ext::MetadataDojoExt;
use starknet::providers::JsonRpcClient;
use starknet::providers::jsonrpc::HttpTransport;
use starknet_crypto::Felt;

use crate::migrate::{Migration, MigrationResult};
use sozo_ui::SozoUi;

/// Sets up the world diff from the environment and returns the world diff used to create a
/// migration.
async fn setup_migration(
    example_project: &str,
    profile: Profile,
    provider: Arc<JsonRpcClient<HttpTransport>>,
) -> Result<WorldDiff> {
    let setup = TestSetup::from_examples("../../dojo/core", "../../../examples/");
    let metadata = setup.load_metadata(example_project, profile);

    let world_local = metadata.load_dojo_world_local().unwrap();
    let world_address = world_local.deterministic_world_address().unwrap();

    let whitelisted_namespaces = vec![];

    let world_diff = WorldDiff::new_from_chain(
        world_address,
        world_local,
        &provider,
        None,
        200_000,
        &whitelisted_namespaces,
    )
    .await?;

    Ok(world_diff)
}

/// Migrates the spawn-and-move project from the local environment.
async fn migrate_spawn_and_move(sequencer: &RunnerCtx, with_metadata: bool) -> MigrationResult {
    let account = sequencer.account(0);
    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    let world_diff = setup_migration("spawn-and-move", Profile::DEV, provider)
        .await
        .expect("Failed to setup migration");

    let world_address = world_diff.world_info.address;
    let profile_config = world_diff.profile_config.clone();

    let is_guest = false;

    let migration = Migration::new(
        world_diff,
        WorldContract::new(world_address, &account),
        TxnConfig::init_wait(),
        profile_config,
        sequencer.url().to_string(),
        is_guest,
    );

    let sozo_ui = SozoUi::default();

    let res = migration.migrate(&sozo_ui).await.expect("Migration spawn-and-move failed.");

    if with_metadata {
        let mut service = MockUploadService::default();
        migration.upload_metadata(&sozo_ui, &mut service).await.expect("Upload metadata failed");
    }

    res
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10)]
async fn migrate_from_local(sequencer: &RunnerCtx) {
    let MigrationResult { manifest, has_changes } = migrate_spawn_and_move(sequencer, false).await;

    assert!(has_changes);
    assert_eq!(manifest.contracts.len(), 5);
    assert_eq!(manifest.external_contracts.len(), 8);
}

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn migrate_no_change(sequencer: &RunnerCtx) {
    let MigrationResult { manifest, has_changes } = migrate_spawn_and_move(sequencer, false).await;

    assert!(!has_changes);
    assert_eq!(manifest.contracts.len(), 5);
}

// helper to check metadata of a list of resources
fn check_resources(
    diff: &WorldDiff,
    resources: Option<Vec<ResourceConfig>>,
    expected_count: usize,
    checker: &dyn Fn(Felt) -> bool,
) {
    assert!(resources.is_some());
    let resources = resources.unwrap();

    assert_eq!(resources.len(), expected_count);

    for resource in resources {
        let selector = dojo_types::naming::compute_selector_from_tag_or_name(&resource.tag);

        let resource = diff.resources.get(&selector);
        assert!(resource.is_some());

        let resource = resource.unwrap();

        assert!(checker(resource.metadata_hash()), "Bad resource hash: {}", resource.name());
    }
}

#[ignore = "Flaky: this test passes when run alone and sometimes when all tests are run, an other \
            test may be cleaning the dev build."]
#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(accounts = 10, db_dir = copy_spawn_and_move_db().as_str())]
async fn upload_metadata(sequencer: &RunnerCtx) {
    let is_set = |hash| hash != Felt::ZERO;
    let is_not_set = |hash: Felt| hash == Felt::ZERO;

    let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(sequencer.url())));

    // here, metadata should not be set
    let world_diff = setup_migration("spawn-and-move", Profile::DEV, provider.clone())
        .await
        .expect("Failed to setup migration");
    let profile_config = world_diff.profile_config.clone();

    assert!(is_not_set(world_diff.world_info.metadata_hash));
    check_resources(&world_diff, profile_config.contracts, 1, &is_not_set);
    check_resources(&world_diff, profile_config.models, 3, &is_not_set);
    check_resources(&world_diff, profile_config.events, 1, &is_not_set);

    // no change is expected for the migration itself but metadata
    // should be uploaded.
    let _ = migrate_spawn_and_move(sequencer, true).await;

    // Note that IPFS upload is deeply tested in dojo-world metadata tests.
    // Here we just check that, after migration, resources associated to
    // metadata configured in dojo_dev.toml have been successfully updated
    // in the `ResourceMetadata` model of the world.
    let world_diff = setup_migration("spawn-and-move", Profile::DEV, provider)
        .await
        .expect("Failed to setup migration");
    let profile_config = world_diff.profile_config.clone();

    // check world and resources metadata from computed WorldDiff
    assert!(is_set(world_diff.world_info.metadata_hash));
    check_resources(&world_diff, profile_config.contracts, 1, &is_set);
    check_resources(&world_diff, profile_config.models, 3, &is_set);
    check_resources(&world_diff, profile_config.events, 1, &is_set);
}
