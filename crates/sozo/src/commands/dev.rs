use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::{anyhow, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::{AsFilesGroupMut, FilesGroupEx, PrivRawFileContentQuery};
use cairo_lang_filesystem::ids::FileId;
use clap::Args;
use dojo_world::manifest::Manifest;
use dojo_world::migration::world::WorldDiff;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebouncedEvent, DebouncedEventKind};
use scarb::compiler::CompilationUnit;
use scarb::core::{Config, Workspace};
use scarb::ui::Status;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use super::scarb_internal::build_scarb_root_database;

#[derive(Args)]
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
                } else {
                    if let Some(extension) = p.extension() {
                        if extension == "cairo" {
                            return DevAction::Build(p.clone());
                        }
                    }
                }
            }
            DevAction::None
        }
        _ => DevAction::None,
    };
    action
}

struct DevContext<'a> {
    pub db: RootDatabase,
    pub unit: CompilationUnit,
    pub ws: Workspace<'a>,
    pub packages: Vec<scarb::core::PackageId>,
}

fn load_context(config: &Config) -> Result<DevContext> {
    let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
    let packages: Vec<scarb::core::PackageId> = ws.members().map(|p| p.id).collect();
    let resolve = scarb::ops::resolve_workspace(&ws)?;
    let compilation_units = scarb::ops::generate_compilation_units(&resolve, &ws)?
        .into_iter()
        .filter(|cu| packages.contains(&cu.main_package_id))
        .collect::<Vec<_>>();
    // we have only 1 unit in projects
    let unit = compilation_units.get(0).unwrap();
    let db = build_scarb_root_database(&unit, &ws).unwrap();
    Ok(DevContext { db, unit: unit.clone(), ws, packages })
}

fn build(context: &mut DevContext) -> Result<()> {
    let ws = &context.ws;
    let packages = context.packages.clone();
    let unit = &context.unit;
    let package_name = unit.main_package_id.name.clone();
    log::error!("compile");
    ws.config().compilers().compile(unit.clone(), &mut ((*context).db), ws).map_err(|err| {
        ws.config().ui().anyhow(&err);

        anyhow!("could not compile `{package_name}` due to previous error")
    })?;
    ws.config().ui().print(Status::new("Rebuild", "done"));
    Ok(())
}

fn migrate(ws: &Workspace<'_>, previous_manifest: Option<Manifest>) -> Result<Manifest> {
    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(ws.config().profile().as_str());
    let manifest_path = target_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(anyhow!("manifest.json not found"));
    }
    let new_manifest = Manifest::load_from_path(manifest_path)?;
    let diff = WorldDiff::compute(new_manifest.clone(), previous_manifest);
    let total_diffs = diff.count_diffs();
    ws.config().ui().print(format!("Total diffs found: {total_diffs}"));
    Ok(new_manifest)
}

impl DevArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let mut context = load_context(config)?;
        let (tx, rx) = channel();
        // Automatically select the best implementation for your platform.

        // No specific tickrate, max debounce time 1 seconds
        let mut debouncer = new_debouncer(Duration::from_secs(1), None, tx)?;

        debouncer.watcher().watch(
            config.manifest_path().parent().unwrap().as_std_path(),
            RecursiveMode::Recursive,
        )?;

        let mut result = build(&mut context);
        let mut previous_manifest: Option<Manifest> = migrate(&context.ws, None).ok();

        loop {
            let mut action = DevAction::None;
            match rx.recv() {
                Ok(events) => {
                    if events.is_ok() {
                        events.unwrap().iter().for_each(|event| {
                            action = handle_event(event);
                            match &action {
                                DevAction::None => {}
                                DevAction::Build(path) => {
                                    context.ws.config().ui().print(Status::new(
                                        "Need to rebuild",
                                        path.clone().as_path().to_str().unwrap(),
                                    ));
                                    let db = &mut context.db;
                                    let file = FileId::new(db, path.clone());
                                    PrivRawFileContentQuery
                                        .in_db_mut(db.as_files_group_mut())
                                        .invalidate(&file);
                                    db.override_file_content(file, None);
                                }
                                DevAction::Reload => {
                                    context
                                        .ws
                                        .config()
                                        .ui()
                                        .print(Status::new("Reloading", "project"));
                                    context = load_context(config).unwrap();
                                }
                            }
                        });
                    }
                }
                Err(error) => {
                    log::error!("Error: {error:?}");
                    break;
                }
            };
            match action {
                DevAction::None => continue,
                _ => (),
            }
            result = build(&mut context);
            match result {
                Ok(_) => {
                    context.ws.config().ui().print("Check migration");
                    previous_manifest = migrate(&context.ws, previous_manifest.clone()).ok();
                }
                Err(_) => {}
            }
        }
        result
    }
}
