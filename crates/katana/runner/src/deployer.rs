use std::env;

use anyhow::{anyhow, Context, Ok, Result};
use clap::Parser;
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use dojo_lang::scarb_internal::compile_workspace;
use dojo_world::manifest::Manifest;
use scarb::compiler::CompilerRepository;
use scarb::core::{Config, TargetKind};
use scarb::ops::CompileOpts;
use sozo::args::{Commands, SozoArgs};
use sozo::ops::migration;
use starknet::core::types::FieldElement;
use tokio::process::Command;

use crate::KatanaRunner;

async fn prepare_migration_args(args: SozoArgs) -> Result<FieldElement> {
    // Preparing config, as in https://github.com/dojoengine/dojo/blob/25fbb7fc973cff4ce1273625c4664545d9b088e9/bin/sozo/src/main.rs#L28-L29
    let mut compilers = CompilerRepository::std();
    let cairo_plugins = CairoPluginRepository::default();
    compilers.add(Box::new(DojoCompiler)).unwrap();
    let manifest_path = scarb::ops::find_manifest_path(args.manifest_path.as_deref())?;

    let config = Config::builder(manifest_path)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .profile(args.profile_spec.determine()?)
        .offline(args.offline)
        .cairo_plugins(cairo_plugins.into())
        .ui_verbosity(args.ui_verbosity())
        .compilers(compilers)
        .build()
        .context("failed to build config")?;

    // Extractiong migration command, as here https://github.com/dojoengine/dojo/blob/25fbb7fc973cff4ce1273625c4664545d9b088e9/bin/sozo/src/commands/mod.rs#L24-L25
    let mut migrate = match args.command {
        Commands::Migrate(migrate) => *migrate,
        _ => return Err(anyhow!("failed to parse migrate args")),
    };

    // Preparing workspace, as in https://github.com/dojoengine/dojo/blob/25fbb7fc973cff4ce1273625c4664545d9b088e9/bin/sozo/src/commands/migrate.rs#L40-L41
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config)?;
    if migrate.name.is_none() {
        if let Some(root_package) = ws.root_package() {
            migrate.name = Some(root_package.id.name.to_string());
        }
    }

    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(ws.config().profile().as_str());

    if !target_dir.join("manifest.json").exists() {
        compile_workspace(
            &config,
            CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
        )?;
    }
    let manifest = Manifest::load_from_path(target_dir.join("manifest.json"))
        .expect("failed to load manifest");

    migration::execute(&ws, migrate, target_dir).await?;
    Ok(manifest.contracts[0].address.unwrap())
}

impl KatanaRunner {
    pub async fn deploy(&self, manifest: &str, script: &str) -> Result<FieldElement> {
        let rpc_url = &format!("http://localhost:{}", self.port);

        let args = SozoArgs::parse_from([
            "sozo",
            "migrate",
            "--rpc-url",
            rpc_url,
            "--manifest-path",
            manifest,
        ]);

        let constract_address = prepare_migration_args(args).await?;

        let out = Command::new("bash")
            .arg(script)
            .env("RPC_URL", rpc_url)
            .output()
            .await
            .context("failed to start script subprocess")?;

        if !out.status.success() {
            return Err(anyhow::anyhow!("script failed {:?}", out));
        }

        Ok(constract_address)
    }
}
