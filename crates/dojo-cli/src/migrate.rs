use std::env::{self, current_dir};
use std::fs;

use anyhow::Context;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use camino::Utf8PathBuf;
use clap::Args;
use comfy_table::Table;
use dojo_project::WorldConfig;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use starknet::core::types::contract::CompiledClass;
use starknet::core::types::FieldElement;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

pub async fn run(args: MigrateArgs) -> anyhow::Result<()> {
    let source_dir = match args.path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                Utf8PathBuf::from_path_buf(current_path).unwrap()
            }
        }
        None => Utf8PathBuf::from_path_buf(current_dir().unwrap()).unwrap(),
    };

    let modules = Modules::from_path(source_dir.clone())?;

    let manifest_path = source_dir.join("Scarb.toml");
    let config = Config::builder(manifest_path)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()
        .unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        std::process::exit(1);
    });
    let world_config = WorldConfig::from_workspace(&ws).unwrap_or_else(|_| WorldConfig::default());

    println!("World Address: 0x{:x}", world_config.address.unwrap());

    let mut components_table = Table::new();
    components_table.set_header(vec!["Component", "Class Hash"]);
    for component in modules.components {
        components_table.add_row(vec![component.name, format!("0x{:x} ", component.hash)]);
    }
    println!("{components_table}\n");

    let mut systems_table = Table::new();
    systems_table.set_header(vec!["System", "Class Hash"]);
    for system in modules.systems {
        systems_table.add_row(vec![system.name, format!("0x{:x} ", system.hash)]);
    }
    println!("{systems_table}");

    Ok(())
}

struct Module {
    name: String,
    hash: FieldElement,
}

struct Modules {
    contracts: Vec<Module>,
    components: Vec<Module>,
    systems: Vec<Module>,
}

impl Modules {
    fn from_path(source_dir: Utf8PathBuf) -> anyhow::Result<Modules> {
        let mut modules = Modules { contracts: vec![], components: vec![], systems: vec![] };

        // Read the directory
        let entries = fs::read_dir(source_dir.join("target/release")).unwrap_or_else(|error| {
            panic!("Problem reading source directory: {:?}", error);
        });

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if !file_name_str.ends_with(".json") {
                continue;
            }

            let name = file_name_str.split('_').last().unwrap().to_string();
            let contract_class = serde_json::from_reader(fs::File::open(entry.path()).unwrap())
                .unwrap_or_else(|error| {
                    panic!("Problem parsing {} artifact: {:?}", file_name_str, error);
                });

            let casm_contract = CasmContractClass::from_contract_class(contract_class, true)
                .with_context(|| "Compilation failed.")?;
            let res = serde_json::to_string_pretty(&casm_contract)
                .with_context(|| "Casm contract Serialization failed.")?;

            let compiled_class: CompiledClass =
                serde_json::from_str(res.as_str()).unwrap_or_else(|error| {
                    panic!("Problem parsing {} artifact: {:?}", file_name_str, error);
                });

            let hash = compiled_class
                .class_hash()
                .with_context(|| "Casm contract Serialization failed.")?;

            if name.ends_with("Component.json") {
                modules.components.push(Module {
                    name: name.strip_suffix("Component.json").unwrap().to_string(),
                    hash,
                });
            } else if name.ends_with("System.json") {
                modules.systems.push(Module {
                    name: name.strip_suffix("System.json").unwrap().to_string(),
                    hash,
                });
            } else {
                modules
                    .contracts
                    .push(Module { name: name.strip_suffix(".json").unwrap().to_string(), hash });
            };
        }
        Ok(modules)
    }
}
