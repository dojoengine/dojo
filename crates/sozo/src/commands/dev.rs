use anyhow::{anyhow, Result};

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::{AsFilesGroupMut, FilesGroupEx, PrivRawFileContentQuery};
use cairo_lang_filesystem::ids::FileId;
use clap::Args;
use notify::event::{AccessKind, AccessMode, ModifyKind, RenameMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use scarb::compiler::CompilationUnit;
use scarb::core::{Config, Workspace};
use scarb::ui::Status;

use std::path::PathBuf;
use std::sync::mpsc::channel;

use super::scarb_internal::build_scarb_root_database;

#[derive(Args, Debug)]
pub struct DevArgs;

enum DevAction {
    None,
    Reload,
    Build(PathBuf),
}

fn handle_event(event: Event) -> DevAction {
    let action = match event.kind {
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        | EventKind::Modify(ModifyKind::Data(_))
        | EventKind::Remove(_) => {
            for p in event.paths.iter() {
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

impl DevArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let mut context = load_context(config)?;
        let (tx, rx) = channel();
        // Automatically select the best implementation for your platform.
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;

        watcher.watch(
            config.manifest_path().parent().unwrap().as_std_path(),
            RecursiveMode::Recursive,
        )?;
        let mut result = build(&mut context);

        loop {
            let mut action = DevAction::None;
            match rx.recv() {
                Ok(event) => {
                    if event.is_ok() {
                        action = handle_event(event.ok().unwrap());
                    }
                }
                Err(error) => {
                    log::error!("Error: {error:?}");
                    break;
                }
            };
            match action {
                DevAction::None => continue,
                DevAction::Build(path) => {
                    context.ws.config().ui().print(Status::new(
                        "Need to rebuild",
                        path.clone().as_path().to_str().unwrap(),
                    ));
                    let db = &mut context.db;
                    let file = FileId::new(db, path);
                    PrivRawFileContentQuery.in_db_mut(db.as_files_group_mut()).invalidate(&file);
                    db.override_file_content(file, None);
                }
                DevAction::Reload => {
                    context.ws.config().ui().print(Status::new("Reloading", "project"));
                    context = load_context(config)?;
                }
            }
            result = build(&mut context);
        }
        result
    }
}
