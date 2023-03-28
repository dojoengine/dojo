use std::env::current_dir;
use std::fs;
use std::path::PathBuf;

use clap::Args;
use comfy_table::Table;
use starknet::core::types::contract::SierraClass;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<PathBuf>,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

struct Module {
    name: String,
    #[allow(clippy::dead_code)]
    artifact: SierraClass,
}

struct Modules {
    contracts: Vec<Module>,
    components: Vec<Module>,
    systems: Vec<Module>,
}

pub async fn run(args: MigrateArgs) {
    let source_dir = match args.path {
        Some(path) => path,
        None => current_dir().unwrap(),
    };

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
        let artifact = serde_json::from_reader(fs::File::open(entry.path()).unwrap())
            .unwrap_or_else(|error| {
                panic!("Problem parsing {} artifact: {:?}", file_name_str, error);
            });

        if name.ends_with("Component.json") {
            modules.components.push(Module {
                name: name.strip_suffix("Component.json").unwrap().to_string(),
                artifact,
            });
        } else if name.ends_with("System.json") {
            modules.systems.push(Module {
                name: name.strip_suffix("System.json").unwrap().to_string(),
                artifact,
            });
        } else {
            modules
                .contracts
                .push(Module { name: name.strip_suffix(".json").unwrap().to_string(), artifact });
        };
    }

    let mut table = Table::new();
    table.set_header(vec!["Name", "Type"]);

    for component in modules.components {
        table.add_row(vec![component.name, "Component".into()]);
    }

    for system in modules.systems {
        table.add_row(vec![system.name, "System".into()]);
    }

    println!("{table}");
}
