// I have copied source code from https://github.com/software-mansion/scarb/blob/main/scarb/src/compiler/db.rs
// since build_scarb_root_database is not public
//
// NOTE: This files needs to be updated whenever scarb version is updated
use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::ids::Directory;
use scarb::compiler::CompilationUnit;
use scarb::core::Workspace;
use tracing::trace;

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
        for macro_plugin in instance.macro_plugins() {
            b.with_macro_plugin(macro_plugin);
        }
        for (name, inline_macro_plugin) in instance.inline_macro_plugins() {
            b.with_inline_macro_plugin(&name, inline_macro_plugin);
        }
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

    let content = ProjectConfigContent { crate_roots };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(?project_config);

    Ok(project_config)
}
