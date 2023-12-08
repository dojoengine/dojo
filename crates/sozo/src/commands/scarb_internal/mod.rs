// I have copied source code from https://github.com/software-mansion/scarb/blob/main/scarb/src/compiler/db.rs
// since build_scarb_root_database is not public
//
// NOTE: This files needs to be updated whenever scarb version is updated
use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_project::{AllCratesConfig, SingleCrateConfig};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use scarb::compiler::CompilationUnit;
use scarb::core::Workspace;
use smol_str::SmolStr;
use tracing::trace;

pub fn crates_config_for_compilation_unit(unit: &CompilationUnit) -> AllCratesConfig {
    let crates_config: OrderedHashMap<SmolStr, SingleCrateConfig> = unit
        .components
        .iter()
        .map(|component| {
            (
                component.cairo_package_name(),
                SingleCrateConfig { edition: component.package.manifest.edition },
            )
        })
        .collect();

    AllCratesConfig { override_map: crates_config, ..Default::default() }
}

// TODO(mkaput): ScarbDatabase?
pub(crate) fn build_scarb_root_database(
    unit: &CompilationUnit,
    ws: &Workspace<'_>,
) -> Result<RootDatabase> {
    let mut b = RootDatabase::builder();
    b.with_project_config(build_project_config(unit)?);
    b.with_cfg(unit.cfg_set.clone());

    for plugin_info in &unit.cairo_plugins {
        let package_id = plugin_info.package.id;
        let plugin = ws.config().cairo_plugins().fetch(package_id)?;
        let instance = plugin.instantiate()?;
        b.with_plugin_suite(instance.plugin_suite());
    }

    b.build()
}

fn build_project_config(unit: &CompilationUnit) -> Result<ProjectConfig> {
    let crate_roots = unit
        .components
        .iter()
        .filter(|model| !model.package.id.is_core())
        .map(|model| (model.cairo_package_name(), model.target.source_root().into()))
        .collect();

    let corelib = Some(Directory::Real(unit.core_package_component().target.source_root().into()));

    let content = ProjectConfigContent {
        crate_roots,
        crates_config: crates_config_for_compilation_unit(unit),
    };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(?project_config);

    Ok(project_config)
}
