use anyhow::{Context, Result};
use clap::Args;
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use dojo_lang::scarb_internal::compile_workspace;
use prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE;
use prettytable::{format, Cell, Row, Table};
use scarb::core::{Config, TargetKind};
use scarb::ops::{CompileOpts, FeaturesOpts, FeaturesSelector};
use sozo_ops::statistics::{get_contract_statistics_for_dir, ContractStatistics};
use tracing::trace;

const SIERRA_BYTECODE_SIZE_LABEL: &str = "[Sierra] Bytecode size (felts)";
const SIERRA_CONTRACT_CLASS_SIZE_LABEL: &str = "[Sierra] Class size (bytes)";

const CASM_BYTECODE_SIZE_LABEL: &str = "[Casm] Bytecode size (felts)";
const CASM_CONTRACT_CLASS_SIZE_LABEL: &str = "[Casm] Class size (bytes)";

const CONTRACT_NAME_LABEL: &str = "Contract";

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript: bool,

    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript_v2: bool,

    #[arg(long)]
    #[arg(help = "Generate Unity bindings.")]
    pub unity: bool,

    #[arg(long)]
    #[arg(help = "Output directory.", default_value = "bindings")]
    pub bindings_output: String,

    #[arg(long, help = "Display statistics about the compiled contracts")]
    pub stats: bool,
}

impl BuildArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let features_opts =
            FeaturesOpts { features: FeaturesSelector::AllFeatures, no_default_features: false };

        let compile_info = compile_workspace(
            config,
            CompileOpts {
                include_targets: vec![],
                exclude_targets: vec![TargetKind::TEST],
                features: features_opts,
            },
        )?;
        trace!(?compile_info, "Compiled workspace.");

        let mut builtin_plugins = vec![];
        if self.typescript {
            builtin_plugins.push(BuiltinPlugins::Typescript);
        }

        if self.typescript_v2 {
            builtin_plugins.push(BuiltinPlugins::TypeScriptV2);
        }

        if self.unity {
            builtin_plugins.push(BuiltinPlugins::Unity);
        }

        if self.stats {
            let target_dir = &compile_info.target_dir;
            let contracts_statistics = get_contract_statistics_for_dir(config.ui(), target_dir)
                .context("Error getting contracts stats")?;
            trace!(
                ?contracts_statistics,
                ?target_dir,
                "Read contract statistics for target directory."
            );

            let table = create_stats_table(contracts_statistics);
            table.printstd()
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
        trace!(pluginManager=?bindgen, "Generating bindings.");

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(bindgen.generate())
            .expect("Error generating bindings");

        Ok(())
    }
}

fn create_stats_table(mut contracts_statistics: Vec<ContractStatistics>) -> Table {
    let mut table = Table::new();
    table.set_format(*FORMAT_NO_LINESEP_WITH_TITLE);

    // Add table headers
    table.set_titles(Row::new(vec![
        Cell::new_align(CONTRACT_NAME_LABEL, format::Alignment::CENTER),
        Cell::new_align(SIERRA_BYTECODE_SIZE_LABEL, format::Alignment::CENTER),
        Cell::new_align(SIERRA_CONTRACT_CLASS_SIZE_LABEL, format::Alignment::CENTER),
        Cell::new_align(CASM_BYTECODE_SIZE_LABEL, format::Alignment::CENTER),
        Cell::new_align(CASM_CONTRACT_CLASS_SIZE_LABEL, format::Alignment::CENTER),
    ]));

    // sort contracts in alphabetical order
    contracts_statistics.sort_by(|a, b| a.contract_name.cmp(&b.contract_name));

    for contract_stats in contracts_statistics {
        // Add table rows
        let contract_name = contract_stats.contract_name;

        let sierra_bytecode_size = contract_stats.sierra_bytecode_size;
        let sierra_contract_class_size = contract_stats.sierra_contract_class_size;

        let casm_bytecode_size = contract_stats.casm_bytecode_size;
        let casm_contract_class_size = contract_stats.casm_contract_class_size;

        table.add_row(Row::new(vec![
            Cell::new_align(&contract_name, format::Alignment::LEFT),
            Cell::new_align(format!("{}", sierra_bytecode_size).as_str(), format::Alignment::RIGHT),
            Cell::new_align(
                format!("{}", sierra_contract_class_size).as_str(),
                format::Alignment::RIGHT,
            ),
            Cell::new_align(format!("{}", casm_bytecode_size).as_str(), format::Alignment::RIGHT),
            Cell::new_align(
                format!("{}", casm_contract_class_size).as_str(),
                format::Alignment::RIGHT,
            ),
        ]));
    }

    table
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use dojo_test_utils::compiler;
    use prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE;
    use prettytable::{format, Cell, Row, Table};
    use sozo_ops::statistics::ContractStatistics;

    use super::{create_stats_table, BuildArgs, *};
    use crate::commands::build::CONTRACT_NAME_LABEL;

    // Uncomment once bindings support arrays.
    #[test]
    fn build_example_with_typescript_and_unity_bindings() {
        let source_project_dir = Utf8PathBuf::from("../../examples/spawn-and-move/");
        let dojo_core_path = Utf8PathBuf::from("../../crates/dojo-core");

        let config = compiler::copy_tmp_config(&source_project_dir, &dojo_core_path);

        let build_args = BuildArgs {
            bindings_output: "generated".to_string(),
            typescript: false,
            unity: true,
            typescript_v2: false,
            stats: true,
        };
        let result = build_args.run(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_stats_table() {
        // Arrange
        let contracts_statistics = vec![
            ContractStatistics {
                contract_name: "Test1".to_string(),
                sierra_bytecode_size: 33,
                sierra_contract_class_size: 33,
                casm_bytecode_size: 66,
                casm_contract_class_size: 66,
            },
            ContractStatistics {
                contract_name: "Test2".to_string(),
                sierra_bytecode_size: 43,
                sierra_contract_class_size: 24,
                casm_bytecode_size: 86,
                casm_contract_class_size: 48,
            },
            ContractStatistics {
                contract_name: "Test3".to_string(),
                sierra_bytecode_size: 36,
                sierra_contract_class_size: 12,
                casm_bytecode_size: 72,
                casm_contract_class_size: 24,
            },
        ];

        let mut expected_table = Table::new();
        expected_table.set_format(*FORMAT_NO_LINESEP_WITH_TITLE);
        expected_table.set_titles(Row::new(vec![
            Cell::new_align(CONTRACT_NAME_LABEL, format::Alignment::CENTER),
            Cell::new_align(SIERRA_BYTECODE_SIZE_LABEL, format::Alignment::CENTER),
            Cell::new_align(SIERRA_CONTRACT_CLASS_SIZE_LABEL, format::Alignment::CENTER),
            Cell::new_align(CASM_BYTECODE_SIZE_LABEL, format::Alignment::CENTER),
            Cell::new_align(CASM_CONTRACT_CLASS_SIZE_LABEL, format::Alignment::CENTER),
        ]));
        expected_table.add_row(Row::new(vec![
            Cell::new_align("Test1", format::Alignment::LEFT),
            Cell::new_align(format!("{}", 33).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 33).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 66).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 66).as_str(), format::Alignment::RIGHT),
        ]));
        expected_table.add_row(Row::new(vec![
            Cell::new_align("Test2", format::Alignment::LEFT),
            Cell::new_align(format!("{}", 43).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 24).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 86).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 48).as_str(), format::Alignment::RIGHT),
        ]));
        expected_table.add_row(Row::new(vec![
            Cell::new_align("Test3", format::Alignment::LEFT),
            Cell::new_align(format!("{}", 36).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 12).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 72).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 24).as_str(), format::Alignment::RIGHT),
        ]));

        // Act
        let table = create_stats_table(contracts_statistics);

        // Assert
        assert_eq!(table, expected_table, "Tables mismatch")
    }
}
