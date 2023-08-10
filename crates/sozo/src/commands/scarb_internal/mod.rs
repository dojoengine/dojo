// I have copied source code from https://github.com/software-mansion/scarb/blob/bf927194941f6c0ce62677e7e2ef4f9122489ff6/scarb/src/compiler/db.rs
// since build_scarb_root_database is not public
use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::ids::Directory;

use scarb::compiler::CompilationUnit;
use scarb::core::Workspace;

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
        for semantic_plugin in instance.semantic_plugins() {
            b.with_semantic_plugin(semantic_plugin);
        }
    }

    b.build()
}

fn build_project_config(unit: &CompilationUnit) -> Result<ProjectConfig> {
    let crate_roots = unit
        .components
        .iter()
        .filter(|component| !component.package.id.is_core())
        .map(|component| {
            (
                component.cairo_package_name(),
                component.target.source_root().into(),
            )
        })
        .collect();

    let corelib = Some(Directory(
        unit.core_package_component().target.source_root().into(),
    ));

    let content = ProjectConfigContent { crate_roots };

    let project_config = ProjectConfig {
        base_path: unit.main_component().package.root().into(),
        corelib,
        content,
    };

    Ok(project_config)
}
