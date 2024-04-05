use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Ok, Result};
use clap::Parser;
use dojo_lang::compiler::{DojoCompiler, DEPLOYMENTS_DIR, MANIFESTS_DIR};
use dojo_lang::plugin::CairoPluginRepository;
use dojo_world::manifest::DeploymentManifest;
use futures::executor::block_on;
use katana_runner::KatanaRunner;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use sozo::args::{Commands, SozoArgs};
use starknet::core::types::FieldElement;
use tokio::process::Command;

use crate::{CONTRACT, CONTRACT_RELATIVE_TO_TESTS, RUNTIME};

pub async fn deploy(runner: &KatanaRunner) -> Result<FieldElement> {
    if let Some(contract) = runner.contract().await {
        return Ok(contract);
    }

    let contract = if PathBuf::from(CONTRACT.0).exists() {
        CONTRACT
    } else {
        if !PathBuf::from(CONTRACT_RELATIVE_TO_TESTS.0).exists() {
            bail!("manifest not found")
        }
        // calls in the `tests` dir use paths relative to itselfs
        CONTRACT_RELATIVE_TO_TESTS
    };

    let address = deploy_contract(runner, contract).await?;
    runner.set_contract(address).await;
    Ok(address)
}

pub fn deploy_sync(runner: &KatanaRunner) -> Result<FieldElement> {
    let _rt = RUNTIME.enter();
    block_on(async move { deploy(runner).await })
}

async fn deploy_contract(
    runner: &KatanaRunner,
    manifest_and_script: (&str, &str),
) -> Result<FieldElement> {
    let args = SozoArgs::parse_from([
        "sozo",
        "migrate",
        "apply",
        "--rpc-url",
        &runner.endpoint(),
        "--manifest-path",
        manifest_and_script.0,
    ]);

    let constract_address = prepare_migration_args(args).await?;

    let out = Command::new("bash")
        .arg(manifest_and_script.1)
        .env("RPC_URL", &runner.endpoint())
        .output()
        .await
        .context("failed to start script subprocess")?;

    if !out.status.success() {
        return Err(anyhow::anyhow!("script failed {:?}", out));
    }

    Ok(constract_address)
}

async fn prepare_migration_args(args: SozoArgs) -> Result<FieldElement> {
    // Preparing config, as in https://github.com/dojoengine/dojo/blob/25fbb7fc973cff4ce1273625c4664545d9b088e9/bin/sozo/src/main.rs#L28-L29
    let mut compilers = CompilerRepository::std();
    let cairo_plugins = CairoPluginRepository::default();
    compilers.add(Box::new(DojoCompiler)).unwrap();
    let manifest_path = scarb::ops::find_manifest_path(args.manifest_path.as_deref())?;

    let config = Config::builder(manifest_path.clone())
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .profile(args.profile_spec.determine()?)
        .offline(args.offline)
        .cairo_plugins(cairo_plugins.into())
        .ui_verbosity(args.ui_verbosity())
        .compilers(compilers)
        .build()
        .context("failed to build config")?;

    // Extractiong migration command, as here https://github.com/dojoengine/dojo/blob/25fbb7fc973cff4ce1273625c4664545d9b088e9/bin/sozo/src/commands/mod.rs#L24-L25
    let migrate = match args.command {
        Commands::Migrate(migrate) => *migrate,
        _ => return Err(anyhow!("failed to parse migrate args")),
    };

    migrate.run(&config)?;

    let manifest_dir = manifest_path.parent().unwrap();

    let manifest = DeploymentManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join("dev").join(DEPLOYMENTS_DIR).with_extension("toml"),
    )
    .expect("failed to load manifest");

    Ok(manifest.contracts[0].inner.address.unwrap())
}
