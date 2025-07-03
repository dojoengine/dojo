use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use anyhow::Result;
use dojo_test_utils::setup::TestSetup;
use dojo_utils::TxnConfig;
use dojo_world::contracts::WorldContract;
use dojo_world::diff::{Manifest, WorldDiff};
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb_interop::Profile;
use scarb_metadata_ext::MetadataDojoExt;
use sozo_ops::migrate::Migration;
use sozo_ops::migration_ui::MigrationUi;
use starknet::core::types::Felt;

async fn migrate_spawn_and_move(db_path: &Path) -> Result<Manifest> {
    println!("migrate spawn_and_move");
    let cfg = KatanaRunnerConfig {
        db_dir: Some(db_path.to_path_buf()),
        n_accounts: 10,
        dev: true,
        ..Default::default()
    };
    let runner = KatanaRunner::new_with_config(cfg)?;

    // setup scarb workspace
    let setup = TestSetup::from_examples("crates/dojo/core", "examples/");
    let metadata = setup.load_metadata("spawn-and-move", Profile::DEV);

    let mut txn_config: TxnConfig = TxnConfig::init_wait();
    txn_config.wait = true;

    let profile_config = metadata.load_dojo_profile_config()?;

    let world_local = metadata.load_dojo_world_local()?;

    // In the case of testing, if the addresses are different it means that the example hasn't been
    // migrated correctly.
    let deterministic_world_address = world_local.deterministic_world_address().unwrap();
    let config_world_address = if let Some(env) = &profile_config.env {
        env.world_address()
            .map_or_else(
                || world_local.deterministic_world_address(),
                |wa| Ok(Felt::from_str(wa).unwrap()),
            )
            .unwrap()
    } else {
        deterministic_world_address
    };

    if deterministic_world_address != config_world_address {
        panic!(
            "The deterministic world address is different from the config world address. Please \
             review the `dojo_dev.toml` file of spawn-and-move example. \nComputed world address: \
             {:#x}",
            deterministic_world_address
        );
    }

    let world_address = deterministic_world_address;

    let whitelisted_namespaces = vec![];
    let world_diff = WorldDiff::new_from_chain(
        world_address,
        world_local,
        &runner.provider(),
        None,
        200_000,
        &whitelisted_namespaces,
    )
    .await?;

    let is_guest = false;

    let result = Migration::new(
        world_diff,
        WorldContract::new(world_address, &runner.account(0)),
        txn_config,
        profile_config,
        runner.url().to_string(),
        is_guest,
    )
    .migrate(&mut MigrationUi::new(None).with_silent())
    .await?;

    Ok(result.manifest)
}

#[tokio::main]
async fn main() -> Result<()> {
    let spawn_and_move_db_path = PathBuf::from("spawn-and-move-db");

    let spawn_and_move_compressed_path = "spawn-and-move-db.tar.gz";

    let _ = fs::remove_dir_all(spawn_and_move_compressed_path);

    // Ensures the db-dir is clean before we start to not include old data.
    // `let _` is used to ignore the result of the remove_dir_all call as it may fail if the
    // directory does not exist.
    let _ = fs::remove_dir_all(&spawn_and_move_db_path);
    fs::create_dir_all(&spawn_and_move_db_path)?;

    let _ = tokio::join!(migrate_spawn_and_move(&spawn_and_move_db_path),);

    // Ensure the test-db directory have been created.
    assert!(spawn_and_move_db_path.exists(), "spawn-and-move-db directory does not exist");

    compress_db(&spawn_and_move_db_path, spawn_and_move_compressed_path);

    assert!(
        PathBuf::from(spawn_and_move_compressed_path).exists(),
        "spawn-and-move-db.tar.gz does not exist"
    );

    Ok(())
}

/// Compresses the given db-path to a .tar.gz file.
fn compress_db(db_path: &Path, compressed_path: &str) {
    Command::new("tar")
        .args(["-czf", compressed_path, "-C", ".", db_path.to_string_lossy().as_ref()])
        .status()
        .expect("Failed to compress test-db directory");
}
