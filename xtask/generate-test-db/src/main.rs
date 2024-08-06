use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_world::migration::TxnConfig;
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb::compiler::Profile;
use sozo_ops::{
    migration::{self, MigrationOutput},
    test_utils,
};
use std::{path::PathBuf, process::Command};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

async fn migrate(runner: &KatanaRunner) -> Result<MigrationOutput> {
    // migrate the example project
    let acc = runner.account(0);

    // setup scarb workspace
    let setup = CompilerTestSetup::from_examples("crates/dojo-core", "examples/");
    let cfg = setup.build_test_config("spawn-and-move", Profile::DEV);
    let ws = scarb::ops::read_workspace(cfg.manifest_path(), &cfg)?;

    // migrate the example project
    let (strat, _) = test_utils::setup::setup_migration(&cfg)?;
    let output = migration::execute_strategy(&ws, &strat, &acc, TxnConfig::init_wait()).await?;

    Ok(output)
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = PathBuf::from("test-db");
    let compressed_path = "test-db.tar.gz";

    // Instantiate Katana with db-dir at ./test-db
    let cfg = KatanaRunnerConfig { db_dir: Some(db_path.clone()), ..Default::default() };
    let runner = KatanaRunner::new_with_config(cfg)?;

    let _ = migrate(&runner).await?;
    drop(runner);

    // ensure the test-db directory have been created
    assert!(db_path.exists(), "test-db directory does not exist");

    // Compress the ./test-db directory using tar
    Command::new("tar")
        .args(&["-czf", compressed_path, "-C", ".", "test-db"])
        .status()
        .expect("Failed to compress test-db directory");

    // ensure the test-db directory have been created
    assert!(PathBuf::from(db_path).exists(), "test-db directory does not exist");

    Ok(())
}
