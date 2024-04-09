use std::fs::File;

use anyhow::Result;
use clap::Args;
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use dojo_lang::scarb_internal::compile_workspace;
use scarb::core::{Config, TargetKind};
use scarb::ops::CompileOpts;

use super::options::statistics::{ContractStatistics, Stats};
use crate::commands::options::statistics::{
    compare_against_limit, get_contract_statistics_for_dir,
};

#[derive(Debug, serde::Deserialize)]
pub struct StarknetConstants {
    max_contract_bytecode_size: u64,
    max_contract_class_size: u64,
}

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript: bool,

    #[arg(long)]
    #[arg(help = "Generate Unity bindings.")]
    pub unity: bool,

    #[arg(long)]
    #[arg(help = "Output directory.", default_value = "bindings")]
    pub bindings_output: String,

    #[command(flatten)]
    pub stats: Stats,
}

impl BuildArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let compile_info: dojo_lang::scarb_internal::CompileInfo = compile_workspace(
            config,
            CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
        )?;

        let mut builtin_plugins = vec![];
        if self.typescript {
            builtin_plugins.push(BuiltinPlugins::Typescript);
        }

        if self.unity {
            builtin_plugins.push(BuiltinPlugins::Unity);
        }

        if self.stats.stats || self.stats.stats_limits.is_some() {
            const STARKNET_CONSTANTS_JSON: &str = "resources/starknet_constants.json";
            let json_consts_path = match &self.stats.stats_limits {
                Some(path) => path.clone(),
                None => String::from(STARKNET_CONSTANTS_JSON),
            };

            let file = File::open(json_consts_path)?;
            let limits: StarknetConstants = serde_json::from_reader(&file)?;
            let contracts_statistics = get_contract_statistics_for_dir(&compile_info.target_dir);

            for contract_stats in contracts_statistics {
                print_stats(contract_stats, &limits);
            }
        }

        // Custom plugins are always empty for now.
        let bindgen = PluginManager {
            profile_name: compile_info.profile_name,
            output_path: self.bindings_output.into(),
            manifest_path: compile_info.manifest_path,
            root_package_name: compile_info
                .root_package_name
                .unwrap_or("NO_ROOT_PACKAGE".to_string()),
            plugins: vec![],
            builtin_plugins,
        };

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(bindgen.generate())
            .expect("Error generating bindings");

        Ok(())
    }
}

pub fn print_stats(contract_statistic: ContractStatistics, limits: &StarknetConstants) {
    println!(
        "---------------Contract Stats for {}---------------",
        contract_statistic.contract_name
    );

    print!(
        "- Contract bytecode size (Number of felts in the program): {}  ",
        contract_statistic.number_felts,
    );
    println!(
        "{}",
        compare_against_limit(contract_statistic.number_felts, limits.max_contract_bytecode_size)
    );

    print!("- Contract Class size: {} bytes  ", contract_statistic.file_size);
    println!(
        "{}",
        compare_against_limit(contract_statistic.file_size, limits.max_contract_class_size)
    );
}

#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler::build_test_config;

    use super::BuildArgs;
    use crate::commands::options::statistics::Stats;

    #[test]
    fn build_example_with_typescript_and_unity_bindings() {
        let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();

        let build_args = BuildArgs {
            bindings_output: "generated".to_string(),
            typescript: true,
            unity: true,
            stats: Stats { stats: false, stats_limits: None },
        };
        let result = build_args.run(&config);
        assert!(result.is_ok());
    }
}
