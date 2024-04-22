use anyhow::{Context, Result};
use clap::Args;
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use dojo_lang::scarb_internal::compile_workspace;
use prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE;
use prettytable::{format, Cell, Row, Table};
use scarb::core::{Config, TargetKind};
use scarb::ops::CompileOpts;
use sozo_ops::statistics::{get_contract_statistics_for_dir, ContractStatistics};

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
        let compile_info = compile_workspace(
            config,
            CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
        )?;

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
            let contracts_statistics = get_contract_statistics_for_dir(target_dir)
                .context("Error getting contracts stats")?;
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

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(bindgen.generate())
            .expect("Error generating bindings");

        Ok(())
    }
}

fn create_stats_table(contracts_statistics: Vec<ContractStatistics>) -> Table {
    let mut table = Table::new();
    table.set_format(*FORMAT_NO_LINESEP_WITH_TITLE);

    // Add table headers
    table.set_titles(Row::new(vec![
        Cell::new_align("Contract", format::Alignment::CENTER),
        Cell::new_align("Bytecode size (felts)", format::Alignment::CENTER),
        Cell::new_align("Class size (bytes)", format::Alignment::CENTER),
    ]));

    for contract_stats in contracts_statistics {
        // Add table rows
        let contract_name = contract_stats.contract_name;
        let number_felts = contract_stats.number_felts;
        let file_size = contract_stats.file_size;

        table.add_row(Row::new(vec![
            Cell::new_align(&contract_name, format::Alignment::LEFT),
            Cell::new_align(format!("{}", number_felts).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", file_size).as_str(), format::Alignment::RIGHT),
        ]));
    }

    table
}

#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler::build_test_config;
    use prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE;
    use prettytable::{format, Cell, Row, Table};
    use sozo_ops::statistics::ContractStatistics;

    use super::{create_stats_table, BuildArgs};

    #[test]
    fn build_example_with_typescript_and_unity_bindings() {
        let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();

        let build_args = BuildArgs {
            bindings_output: "generated".to_string(),
            typescript: true,
            unity: true,
            typescript_v2: true,
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
                number_felts: 33,
                file_size: 33,
            },
            ContractStatistics {
                contract_name: "Test2".to_string(),
                number_felts: 43,
                file_size: 24,
            },
            ContractStatistics {
                contract_name: "Test3".to_string(),
                number_felts: 36,
                file_size: 12,
            },
        ];

        let mut expected_table = Table::new();
        expected_table.set_format(*FORMAT_NO_LINESEP_WITH_TITLE);
        expected_table.set_titles(Row::new(vec![
            Cell::new_align("Contract", format::Alignment::CENTER),
            Cell::new_align("Bytecode size (felts)", format::Alignment::CENTER),
            Cell::new_align("Class size (bytes)", format::Alignment::CENTER),
        ]));
        expected_table.add_row(Row::new(vec![
            Cell::new_align("Test1", format::Alignment::LEFT),
            Cell::new_align(format!("{}", 33).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 33).as_str(), format::Alignment::RIGHT),
        ]));
        expected_table.add_row(Row::new(vec![
            Cell::new_align("Test2", format::Alignment::LEFT),
            Cell::new_align(format!("{}", 43).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 24).as_str(), format::Alignment::RIGHT),
        ]));
        expected_table.add_row(Row::new(vec![
            Cell::new_align("Test3", format::Alignment::LEFT),
            Cell::new_align(format!("{}", 36).as_str(), format::Alignment::RIGHT),
            Cell::new_align(format!("{}", 12).as_str(), format::Alignment::RIGHT),
        ]));

        // Act
        let table = create_stats_table(contracts_statistics);

        // Assert
        assert_eq!(table, expected_table, "Tables mismatch")
    }
}
