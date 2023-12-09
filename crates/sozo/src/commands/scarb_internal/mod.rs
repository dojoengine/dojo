// I have copied source code from https://github.com/software-mansion/scarb/blob/main/scarb/src/compiler/db.rs
// since build_scarb_root_database is not public
//
// NOTE: This files needs to be updated whenever scarb version is updated
use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_project::{AllCratesConfig, SingleCrateConfig};
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_test_plugin::test_plugin_suite;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use dojo_lang::plugin::dojo_plugin_suite;
use scarb::compiler::CompilationUnit;
use scarb::core::Config;
use scarb::ops::CompileOpts;
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
pub(crate) fn build_scarb_root_database(unit: &CompilationUnit) -> Result<RootDatabase> {
    let mut b = RootDatabase::builder();
    b.with_project_config(build_project_config(unit)?);
    b.with_cfg(unit.cfg_set.clone());

    // TODO: Is it fair to consider only those plugins at the moment?
    b.with_plugin_suite(test_plugin_suite());
    b.with_plugin_suite(dojo_plugin_suite());
    b.with_plugin_suite(starknet_plugin_suite());

    b.build()
}

pub(crate) fn compile_workspace(config: &Config, opts: CompileOpts) -> Result<()> {
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

        match ws.config().compilers().compile(unit.clone(), &mut (db), &ws) {
            Err(err) => ws.config().ui().anyhow(&err),
            Ok(_) => (),
        }
    }

    Ok(())
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
