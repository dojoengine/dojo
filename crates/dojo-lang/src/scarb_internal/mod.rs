// I have copied source code from https://github.com/software-mansion/scarb/blob/main/scarb/src/compiler/db.rs
// since build_scarb_root_database is not public.
//
// NOTE: This files needs to be updated whenever scarb version is updated.
// NOTE: This file was moved here from `sozo` as we need to compile here too,
//       and `sozo` has `dojo-lang` as dependency.
use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_filesystem::db::{CrateSettings, ExperimentalFeaturesConfig};
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_project::AllCratesConfig;
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_test_plugin::test_plugin_suite;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use camino::Utf8PathBuf;
use dojo_world::config::{
    NamespaceConfig, DEFAULT_NAMESPACE_CFG_KEY, DOJO_MANIFESTS_DIR_CFG_KEY, NAMESPACE_CFG_PREFIX,
};
use dojo_world::metadata::dojo_metadata_from_package;
use regex::Regex;
use scarb::compiler::{
    CairoCompilationUnit, CompilationUnit, CompilationUnitAttributes, CompilationUnitComponent,
};
use scarb::core::{Config, PackageId, Workspace};
use scarb::ops::CompileOpts;
use scarb_ui::Ui;
use smol_str::SmolStr;
use tracing::trace;

use crate::plugin::dojo_plugin_suite;

pub(crate) const LOG_TARGET: &str = "dojo_lang::scarb_internal";
pub mod debug;

/// Compilation information of all the units found in the workspace.
#[derive(Debug, Default)]
pub struct CompileInfo {
    /// The name of the profile used to compile.
    pub profile_name: String,
    /// The path to the manifest file.
    pub manifest_path: Utf8PathBuf,
    /// The path to the target directory.
    pub target_dir: Utf8PathBuf,
    /// The name of the root package.
    pub root_package_name: Option<String>,
    /// The list of units that failed to compile.
    pub compile_error_units: Vec<String>,
}

pub fn crates_config_for_compilation_unit(unit: &CairoCompilationUnit) -> AllCratesConfig {
    let crates_config: OrderedHashMap<SmolStr, CrateSettings> = unit
        .components()
        .iter()
        .map(|component| {
            // Ensure experimental features are only enable if required.
            let experimental_features = component.package.manifest.experimental_features.clone();
            let experimental_features = experimental_features.unwrap_or_default();

            (
                component.cairo_package_name(),
                CrateSettings {
                    edition: component.package.manifest.edition,
                    experimental_features: ExperimentalFeaturesConfig {
                        negative_impls: experimental_features
                            .contains(&SmolStr::new_inline("negative_impls")),
                        coupons: experimental_features.contains(&SmolStr::new_inline("coupons")),
                    },
                    cfg_set: component.cfg_set.clone(),
                },
            )
        })
        .collect();

    AllCratesConfig { override_map: crates_config, ..Default::default() }
}

/// Builds the scarb root database injecting the dojo plugin suite, additionaly to the
/// default Starknet and Test suites.
pub fn build_scarb_root_database(unit: &CairoCompilationUnit) -> Result<RootDatabase> {
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
pub fn compile_workspace(
    config: &Config,
    opts: CompileOpts,
    packages: Vec<PackageId>,
) -> Result<CompileInfo> {
    let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
    let resolve = scarb::ops::resolve_workspace(&ws)?;
    let ui = config.ui();

    let compilation_units = scarb::ops::generate_compilation_units(&resolve, &opts.features, &ws)?
        .into_iter()
        .filter(|cu| !opts.exclude_target_kinds.contains(&cu.main_component().target_kind()))
        .filter(|cu| {
            opts.include_target_kinds.is_empty()
                || opts.include_target_kinds.contains(&cu.main_component().target_kind())
        })
        .filter(|cu| packages.contains(&cu.main_package_id()))
        .collect::<Vec<_>>();

    let mut compile_error_units = vec![];

    for unit in compilation_units {
        trace!(target: LOG_TARGET, unit_name = %unit.name(), target_kind = %unit.main_component().target_kind(), "Compiling unit.");

        // Proc macro are not supported yet on Dojo, hence we only consider processing Cairo
        // compilation units.
        if let CompilationUnit::Cairo(mut unit) = unit {
            let unit_name = unit.name();
            let re = Regex::new(r"\s*\([^()]*\)$").unwrap();
            let unit_name_no_path = re.replace(&unit_name, "");

            ui.print(format!("compiling {}", unit_name_no_path));
            ui.verbose(format!("target kind: {}", unit.main_component().target_kind()));

            let dojo_metadata = dojo_metadata_from_package(&unit.components[0].package, &ws)?;

            // let root_package_data =
            // PackageData::from_scarb_package(&unit.components[0].package)?;

            ui.verbose(dojo_metadata.namespace.display_mappings());

            // For each component in the compilation unit (namely, the dependencies being
            // compiled) we inject into the `CfgSet` the component name and
            // namespace configuration. Doing this here ensures the parsing of
            // of the manifest is done once at compile time, and not everytime
            // the plugin is called.
            for c in unit.components.iter_mut() {
                c.cfg_set = Some(cfg_set_from_component(c, &dojo_metadata.namespace, &ui, &ws)?);
            }

            let mut db = build_scarb_root_database(&unit).unwrap();
            if let Err(err) = ws.config().compilers().compile(unit.clone(), &mut (db), &ws) {
                ws.config().ui().anyhow(&err);
                compile_error_units.push(unit.name());
            }
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

    Ok(CompileInfo {
        manifest_path,
        target_dir,
        root_package_name,
        profile_name,
        compile_error_units,
    })
}

fn build_project_config(unit: &CairoCompilationUnit) -> Result<ProjectConfig> {
    let crate_roots = unit
        .components()
        .iter()
        .filter(|model| !model.package.id.is_core())
        // NOTE: We're taking the first target of each compilation unit, which should always be the
        //       main package source root due to the order maintained by scarb.
        .map(|model| (model.cairo_package_name(), model.targets[0].source_root().into()))
        .collect();

    let corelib =
        // NOTE: We're taking the first target of the corelib, which should always be the
        //       main package source root due to the order maintained by scarb.
        unit.core_package_component().map(|c| Directory::Real(c.targets[0].source_root().into()));

    let content = ProjectConfigContent {
        crate_roots,
        crates_config: crates_config_for_compilation_unit(unit),
    };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(target: LOG_TARGET, ?project_config);

    Ok(project_config)
}

pub fn cfg_set_from_component(
    c: &CompilationUnitComponent,
    root_package_namespace: &NamespaceConfig,
    ui: &Ui,
    ws: &Workspace<'_>,
) -> Result<CfgSet> {
    let cname = c.cairo_package_name().clone();
    let package_dojo_metadata = dojo_metadata_from_package(&c.package, ws)?;

    let dojo_manifests_dir = ws
        .config()
        .manifest_path()
        .parent()
        .expect("Scarb.toml manifest should always have parent")
        .join("manifests")
        .join(ws.current_profile().expect("profile should be set").to_string());

    ui.verbose(format!("component: {} ({})", cname, c.package.manifest_path()));

    tracing::debug!(target: LOG_TARGET, ?c, ?package_dojo_metadata);

    let component_cfg = Cfg { key: "component_name".into(), value: Some(cname) };

    let mut cfg_set = CfgSet::new();

    // Keep orinigal cfg set of the component.
    if let Some(component_cfg_set) = c.cfg_set.clone() {
        for cfg in component_cfg_set.into_iter() {
            cfg_set.insert(cfg);
        }
    }

    // Add it's name for debugging on the plugin side.
    cfg_set.insert(component_cfg);

    cfg_set.insert(Cfg {
        key: DOJO_MANIFESTS_DIR_CFG_KEY.into(),
        value: Some(dojo_manifests_dir.to_string().into()),
    });

    cfg_set.insert(Cfg {
        key: DEFAULT_NAMESPACE_CFG_KEY.into(),
        value: Some(package_dojo_metadata.namespace.default.into()),
    });

    // We ignore mappings for dependencies as the [[target.dojo]] package is
    // defining them.

    // Inject the mapping from the root package with [[target.dojo]] to
    // all dependencies to ensure correct namespace mappings.
    if let Some(mappings) = &root_package_namespace.mappings {
        for (k, v) in mappings.iter() {
            cfg_set.insert(Cfg {
                key: format!("{}{}", NAMESPACE_CFG_PREFIX, k).into(),
                value: Some(v.into()),
            });
        }
    }

    Ok(cfg_set)
}
