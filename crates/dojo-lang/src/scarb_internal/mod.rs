// I have copied source code from https://github.com/software-mansion/scarb/blob/main/scarb/src/compiler/db.rs
// since build_scarb_root_database is not public.
//
// NOTE: This files needs to be updated whenever scarb version is updated.
// NOTE: This file was moved here from `sozo` as we need to compile here too,
//       and `sozo` has `dojo-lang` as dependency.
use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::db::CrateSettings;
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_project::AllCratesConfig;
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_test_plugin::test_plugin_suite;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use camino::Utf8PathBuf;
use scarb::compiler::CompilationUnit;
use scarb::core::Config;
use scarb::ops::CompileOpts;
use smol_str::SmolStr;
use tracing::trace;

use crate::plugin::dojo_plugin_suite;

pub(crate) const LOG_TARGET: &str = "dojo_lang::scarb_internal";

#[derive(Debug)]
pub struct CompileInfo {
    pub profile_name: String,
    pub manifest_path: Utf8PathBuf,
    pub target_dir: Utf8PathBuf,
    pub root_package_name: Option<String>,
}

pub fn crates_config_for_compilation_unit(unit: &CompilationUnit) -> AllCratesConfig {
    let crates_config: OrderedHashMap<SmolStr, CrateSettings> = unit
        .components
        .iter()
        .map(|component| {
            (
                component.cairo_package_name(),
                CrateSettings { edition: component.package.manifest.edition, ..Default::default() },
            )
        })
        .collect();

    AllCratesConfig { override_map: crates_config, ..Default::default() }
}

/// Builds the scarb root database injecting the dojo plugin suite, additionaly to the
/// default Starknet and Test suites.
pub fn build_scarb_root_database(unit: &CompilationUnit) -> Result<RootDatabase> {
    let mut b = RootDatabase::builder();
    b.with_project_config(build_project_config(unit)?);
    b.with_cfg(unit.cfg_set.clone());

    b.with_plugin_suite(test_plugin_suite());
    b.with_plugin_suite(dojo_plugin_suite());
    b.with_plugin_suite(starknet_plugin_suite());

    b.build()
}

/// This function is an alternative to `ops::compile`, it's doing the same job.
/// However, we can control the injection of the plugins, required to have dojo plugin present
/// for each compilation.
pub fn compile_workspace(config: &Config, opts: CompileOpts) -> Result<CompileInfo> {
    let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
    let packages: Vec<scarb::core::PackageId> = ws.members().map(|p| p.id).collect();
    let resolve = scarb::ops::resolve_workspace(&ws)?;
    let compilation_units = scarb::ops::generate_compilation_units(&resolve, &ws)?
        .into_iter()
        .filter(|cu| !opts.exclude_targets.contains(&cu.target().kind))
        .filter(|cu| {
            opts.include_targets.is_empty() || opts.include_targets.contains(&cu.target().kind)
        })
        .filter(|cu| packages.contains(&cu.main_package_id))
        .collect::<Vec<_>>();

    for unit in compilation_units {
        let mut db = build_scarb_root_database(&unit).unwrap();

        if let Err(err) = ws.config().compilers().compile(unit.clone(), &mut (db), &ws) {
            ws.config().ui().anyhow(&err)
        }
    }

    let manifest_path = ws.manifest_path().into();
    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(ws.config().profile().as_str());

    // The root package may be non existent in a scarb project/workspace.
    // Please refer here:
    let root_package_name = if let Some(package) = ws.root_package() {
        Some(package.id.name.to_string())
    } else {
        None
    };

    let profile_name =
        if let Ok(p) = ws.current_profile() { p.to_string() } else { "NO_PROFILE".to_string() };

    Ok(CompileInfo { manifest_path, target_dir, root_package_name, profile_name })
}

fn build_project_config(unit: &CompilationUnit) -> Result<ProjectConfig> {
    let crate_roots = unit
        .components
        .iter()
        .filter(|model| !model.package.id.is_core())
        .map(|model| (model.cairo_package_name(), model.target.source_root().into()))
        .collect();

    let corelib =
        unit.core_package_component().map(|c| Directory::Real(c.target.source_root().into()));

    let content = ProjectConfigContent {
        crate_roots,
        crates_config: crates_config_for_compilation_unit(unit),
    };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(target: LOG_TARGET, ?project_config);

    Ok(project_config)
}
