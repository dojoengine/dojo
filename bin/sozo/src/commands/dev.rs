use std::mem;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::{anyhow, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::{AsFilesGroupMut, FilesGroupEx, PrivRawFileContentQuery};
use cairo_lang_filesystem::ids::FileId;
use clap::Args;
use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR};
use dojo_lang::scarb_internal::build_scarb_root_database;
use dojo_world::manifest::{BaseManifest, DeploymentManifest};
use dojo_world::metadata::dojo_metadata_from_workspace;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::TxnConfig;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind};
use scarb::compiler::CompilationUnit;
use scarb::core::{Config, Workspace};
use sozo_ops::migration;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::FieldElement;
use starknet::providers::Provider;
use starknet::signers::Signer;
use tracing::{error, trace};

use super::migrate::setup_env;
use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;

#[derive(Debug, Args)]
pub struct DevArgs {
    #[arg(long)]
    #[arg(help = "Name of the World.")]
    #[arg(long_help = "Name of the World. It's hash will be used as a salt when deploying the \
                       contract to avoid address conflicts.")]
    pub name: Option<String>,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,
}

impl DevArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let env_metadata = if config.manifest_path().exists() {
            dojo_metadata_from_workspace(&ws).env().cloned()
        } else {
            trace!("Manifest path does not exist.");
            None
        };

        let mut context = load_context(config)?;

        let (tx, rx) = channel();
        let mut debouncer = new_debouncer(Duration::from_secs(1), None, tx)?;

        debouncer.watcher().watch(
            config.manifest_path().parent().unwrap().as_std_path(),
            RecursiveMode::Recursive,
        )?;

        let name = self.name.unwrap_or_else(|| ws.root_package().unwrap().id.name.to_string());

        let mut previous_manifest: Option<DeploymentManifest> = Option::None;
        let result = build(&mut context);

        let Some((mut world_address, account, _)) = context
            .ws
            .config()
            .tokio_handle()
            .block_on(setup_env(
                &context.ws,
                self.account,
                self.starknet,
                self.world,
                &name,
                env_metadata.as_ref(),
            ))
            .ok()
        else {
            return Err(anyhow!("Failed to setup environment."));
        };

        match context.ws.config().tokio_handle().block_on(migrate(
            world_address,
            &account,
            &name,
            &context.ws,
            previous_manifest.clone(),
        )) {
            Ok((manifest, address)) => {
                previous_manifest = Some(manifest);
                world_address = address;
            }
            Err(error) => {
                error!(
                    error = ?error,
                    address = ?world_address,
                    "Migrating world."
                );
            }
        }
        loop {
            let action = match rx.recv() {
                Ok(Ok(events)) => events
                    .iter()
                    .map(|event| process_event(event, &mut context))
                    .last()
                    .unwrap_or(DevAction::None),
                Ok(Err(_)) => DevAction::None,
                Err(error) => {
                    error!(error = ?error, "Receiving dev action.");
                    break;
                }
            };

            if action != DevAction::None && build(&mut context).is_ok() {
                match context.ws.config().tokio_handle().block_on(migrate(
                    world_address,
                    &account,
                    &name,
                    &context.ws,
                    previous_manifest.clone(),
                )) {
                    Ok((manifest, address)) => {
                        previous_manifest = Some(manifest);
                        world_address = address;
                    }
                    Err(error) => {
                        error!(
                            error = ?error,
                            address = ?world_address,
                            "Migrating world.",
                        );
                    }
                }
            }
        }
        result
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum DevAction {
    None,
    Reload,
    Build(PathBuf),
}

fn handle_event(event: &DebouncedEvent) -> DevAction {
    let action = match event.kind {
        DebouncedEventKind::Any => {
            let p = event.path.clone();
            if let Some(filename) = p.file_name() {
                if filename == "Scarb.toml" {
                    return DevAction::Reload;
                } else if let Some(extension) = p.extension() {
                    if extension == "cairo" {
                        return DevAction::Build(p.clone());
                    }
                }
            }
            DevAction::None
        }
        _ => DevAction::None,
    };

    trace!(?action, "Determined action.");
    action
}

struct DevContext<'a> {
    pub db: RootDatabase,
    pub unit: CompilationUnit,
    pub ws: Workspace<'a>,
}

fn load_context(config: &Config) -> Result<DevContext<'_>> {
    let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
    let packages: Vec<scarb::core::PackageId> = ws.members().map(|p| p.id).collect();
    let resolve = scarb::ops::resolve_workspace(&ws)?;
    let compilation_units = scarb::ops::generate_compilation_units(&resolve, &ws)?
        .into_iter()
        .filter(|cu| packages.contains(&cu.main_package_id))
        .collect::<Vec<_>>();

    // we have only 1 unit in projects
    // TODO: double check if we always have one with the new version and the order if many.
    trace!(unit_count = compilation_units.len(), "Gathering compilation units.");
    let unit = compilation_units.first().unwrap();
    let db = build_scarb_root_database(unit).unwrap();
    Ok(DevContext { db, unit: unit.clone(), ws })
}

fn build(context: &mut DevContext<'_>) -> Result<()> {
    let ws = &context.ws;
    let unit = &context.unit;
    let package_name = unit.main_package_id.name.clone();
    ws.config().compilers().compile(unit.clone(), &mut (context.db), ws).map_err(|err| {
        ws.config().ui().anyhow(&err);

        anyhow!("could not compile `{package_name}` due to previous error")
    })?;
    ws.config().ui().print("📦 Rebuild done");
    Ok(())
}

async fn migrate<P, S>(
    mut world_address: Option<FieldElement>,
    account: &SingleOwnerAccount<P, S>,
    name: &str,
    ws: &Workspace<'_>,
    previous_manifest: Option<DeploymentManifest>,
) -> Result<(DeploymentManifest, Option<FieldElement>)>
where
    P: Provider + Sync + Send + 'static,
    S: Signer + Sync + Send + 'static,
{
    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(ws.config().profile().as_str());

    // `parent` returns `None` only when its root path, so its safe to unwrap
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();

    if !manifest_dir.join(MANIFESTS_DIR).exists() {
        return Err(anyhow!("Build project using `sozo build` first"));
    }

    let new_manifest =
        BaseManifest::load_from_path(&manifest_dir.join(MANIFESTS_DIR).join(BASE_DIR))?;

    let diff = WorldDiff::compute(new_manifest.clone(), previous_manifest);
    let total_diffs = diff.count_diffs();
    let config = ws.config();
    config.ui().print(format!("Total diffs found: {total_diffs}"));
    if total_diffs == 0 {
        return Ok((new_manifest.into(), world_address));
    }

    let ui = ws.config().ui();
    let mut strategy = migration::prepare_migration(&target_dir, diff, name, world_address, &ui)?;

    match migration::apply_diff(ws, account, TxnConfig::default(), &mut strategy).await {
        Ok(migration_output) => {
            config.ui().print(format!(
                "🎉 World at address {} updated!",
                format_args!("{:#x}", migration_output.world_address)
            ));
            world_address = Some(migration_output.world_address);
        }
        Err(err) => {
            config.ui().error(err.to_string());
            return Err(err);
        }
    }

    Ok((new_manifest.into(), world_address))
}

fn process_event(event: &DebouncedEvent, context: &mut DevContext<'_>) -> DevAction {
    trace!(event=?event, "Processing event.");
    let action = handle_event(event);
    match &action {
        DevAction::None => {}
        DevAction::Build(path) => handle_build_action(path, context),
        DevAction::Reload => {
            handle_reload_action(context);
        }
    }

    trace!(action=?action, "Processed action.");
    action
}

fn handle_build_action(path: &Path, context: &mut DevContext<'_>) {
    context
        .ws
        .config()
        .ui()
        .print(format!("📦 Need to rebuild {}", path.to_str().unwrap_or_default(),));
    let db = &mut context.db;
    let file = FileId::new(db, path.to_path_buf());
    PrivRawFileContentQuery.in_db_mut(db.as_files_group_mut()).invalidate(&file);
    db.override_file_content(file, None);
}

fn handle_reload_action(context: &mut DevContext<'_>) {
    trace!("Reloading context.");
    let config = context.ws.config();
    config.ui().print("Reloading project");
    let new_context = load_context(config).expect("Failed to load context");
    let _ = mem::replace(context, new_context);
    trace!("Context reloaded.");
}
