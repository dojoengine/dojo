//! Compiles and runs tests for a Dojo project.

use include_dir::{include_dir, Dir};

use std::path::PathBuf;

use anyhow::Result;

use camino::Utf8PathBuf;
use clap::Args;

use tempfile::tempdir;

use forge::scarb::{get_contracts_map, try_get_starknet_artifacts_path};
use forge::{run, RunnerConfig};

use scarb::core::Config;
use scarb::ops;

use scarb_metadata::MetadataCommand;

static PREDEPLOYED_CONTRACTS: Dir = include_dir!("crates/snforge-predeployed-contracts");
/// Execute all unit tests of a local package.
#[derive(Args, Clone)]
pub struct TestArgs {
    /// The filter for the tests, running only tests containing the filter string.
    #[arg(short, long, default_value_t = String::default())]
    filter: String,
    /// Should we run ignored tests as well.
    #[arg(long, default_value_t = false)]
    include_ignored: bool,
    /// Should we run only the ignored tests.
    #[arg(long, default_value_t = false)]
    ignored: bool,
}

impl TestArgs {
    pub fn run(self, config: &Config) -> anyhow::Result<()> {
        println!("Config path {}", config.manifest_path());
        // Workspace
        let ws = ops::read_workspace(config.manifest_path(), config).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });

        // Build artifacts
        sozo_build(config)?;

        // Corelib
        let resolve = ops::resolve_workspace(&ws)?;
        let compilation_units = ops::generate_compilation_units(&resolve, &ws)?;
        let corelib =
            Utf8PathBuf::from(compilation_units[0].core_package_component().target.source_root());

        // let corelib = Utf8PathBuf::from_path_buf(load_files_in_dir(&CORELIB_PATH).unwrap())
        //     .expect("Failed to prepare corelib");

        // Foundry friendly metadata
        let scarb_metadata =
            MetadataCommand::new().manifest_path(config.manifest_path()).inherit_stderr().exec()?;

        let predeployed_contracts =
            Utf8PathBuf::from_path_buf(load_files_in_dir(&PREDEPLOYED_CONTRACTS).unwrap())
                .expect("Failed to prepare cheats");

        for package in &scarb_metadata.workspace.members {
            let forge_config =
                forge::scarb::config_from_scarb_for_package(&scarb_metadata, package)?;

            let (package_path, lib_path, _corelib_path, dependencies, target_name) =
                forge::scarb::dependencies_for_package(&scarb_metadata, package)?;
            let contracts_path = try_get_starknet_artifacts_path(&package_path, &target_name)?;
            let contracts = contracts_path
                .map(|path| get_contracts_map(&path))
                .transpose()?
                .unwrap_or_default();

            let runner_config = RunnerConfig::new(
                if self.filter.len() > 0 { Some(self.filter.clone()) } else { None },
                false,
                false,
                &forge_config,
            );

            println!("--------------- RUN ARGS ---------------");
            println!("package_path: {package_path}");
            println!("lib_path: {lib_path}");
            println!("dependencies: {dependencies:?}");
            println!("runner_config: {runner_config:?}");
            println!("corelib: {corelib}");
            println!("contracts: HashMap<String, StarknetContractArtifacts>");
            println!("predeployed_contracts: {predeployed_contracts}");

            run(
                &package_path,
                &lib_path,
                &Some(dependencies.clone()),
                &runner_config,
                Some(&corelib),
                &contracts,
                &predeployed_contracts,
            )?;
        }
        println!("Thats all packages.");

        Ok(())
    }
}

// Essentially main for Commands::Build(_)
// DojoCompiler added for Command::Test
fn sozo_build(config: &Config) -> Result<()> {
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config)?;
    ops::compile(&ws)
}

fn load_files_in_dir(files: &Dir) -> Result<PathBuf> {
    let tmp_dir = tempdir()?;
    files.extract(&tmp_dir)?;
    Ok(tmp_dir.path().into())
}
