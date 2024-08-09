use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_world::migration::TxnConfig;
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb::compiler::Profile;
use sozo_ops::migration::{self, MigrationOutput};
use sozo_ops::test_utils;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

async fn migrate_spawn_and_move(db_path: &Path) -> Result<MigrationOutput> {
    let cfg = KatanaRunnerConfig { db_dir: Some(db_path.to_path_buf()), ..Default::default() };
    let runner = KatanaRunner::new_with_config(cfg)?;

    // migrate the example project
    let acc = runner.account(0);

    // setup scarb workspace
    let setup = CompilerTestSetup::from_examples("crates/dojo-core", "examples/");
    let cfg = setup.build_test_config("spawn-and-move", Profile::DEV);
    let ws = scarb::ops::read_workspace(cfg.manifest_path(), &cfg)?;

    // migrate the example project
    let (strat, _) = test_utils::setup::setup_migration(&cfg, "dojo_examples")?;
    let output = migration::execute_strategy(&ws, &strat, &acc, TxnConfig::init_wait()).await?;

    Ok(output)
}

async fn migrate_types_test(db_path: &Path) -> Result<MigrationOutput> {
    let cfg = KatanaRunnerConfig { db_dir: Some(db_path.to_path_buf()), ..Default::default() };
    let runner = KatanaRunner::new_with_config(cfg)?;

    // migrate the example project
    let acc = runner.account(0);

    // setup scarb workspace
    let setup = CompilerTestSetup::from_paths("crates/dojo-core", &["crates/torii/types-test"]);
    let cfg = setup.build_test_config("types-test", Profile::DEV);
    let ws = scarb::ops::read_workspace(cfg.manifest_path(), &cfg)?;

    // migrate the example project
    let (strat, _) = test_utils::setup::setup_migration(&cfg, "types_test")?;
    let output = migration::execute_strategy(&ws, &strat, &acc, TxnConfig::init_wait()).await?;

    Ok(output)
}

#[tokio::main]
async fn main() -> Result<()> {
    let spawn_and_move_db_path = PathBuf::from("spawn-and-move-db");
    let types_test_db_path = PathBuf::from("types-test-db");

    let spawn_and_move_compressed_path = "spawn-and-move-db.tar.gz";
    let types_test_compressed_path = "types-test-db.tar.gz";

    let _ = fs::remove_dir_all(spawn_and_move_compressed_path);
    let _ = fs::remove_dir_all(types_test_compressed_path);

    // Ensures the db-dir is clean before we start to not include old data.
    // `let _` is used to ignore the result of the remove_dir_all call as it may fail if the
    // directory does not exist.
    let _ = fs::remove_dir_all(&spawn_and_move_db_path);
    fs::create_dir_all(&spawn_and_move_db_path)?;
    let _ = fs::remove_dir_all(&types_test_db_path);
    fs::create_dir_all(&types_test_db_path)?;

    let (_, _) = tokio::join!(
        migrate_spawn_and_move(&spawn_and_move_db_path),
        migrate_types_test(&types_test_db_path)
    );

    // Ensure the test-db directory have been created.
    assert!(spawn_and_move_db_path.exists(), "spawn-and-move-db directory does not exist");
    assert!(types_test_db_path.exists(), "types-test-db directory does not exist");

    compress_db(&spawn_and_move_db_path, spawn_and_move_compressed_path);
    compress_db(&types_test_db_path, types_test_compressed_path);

    assert!(
        PathBuf::from(spawn_and_move_compressed_path).exists(),
        "spawn-and-move-db.tar.gz does not exist"
    );
    assert!(
        PathBuf::from(types_test_compressed_path).exists(),
        "types-test-db.tar.gz does not exist"
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
