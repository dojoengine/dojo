use std::env::current_dir;
use std::path::PathBuf;

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::get_main_crate_ids_from_project;
use clap::Args;
use comfy_table::Table;
use dojo_lang::component::find_components;
use dojo_lang::db::DojoRootDatabaseBuilderEx;
use dojo_lang::plugin::get_contract_address;
use dojo_lang::system::find_systems;
use dojo_project::ProjectConfig;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use url::Url;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<PathBuf>,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

pub fn run(args: MigrateArgs) {
    let source_dir = match args.path {
        Some(path) => path,
        None => current_dir().unwrap(),
    };

    let config = ProjectConfig::from_directory(&source_dir).unwrap_or_else(|error| {
        panic!("Problem creating project config: {:?}", error);
    });

    let rpc_client = JsonRpcClient::new(HttpTransport::new(
        Url::parse("https://starknet-goerli.cartridge.gg/").unwrap(),
    ));

    let db = &mut RootDatabase::builder().with_dojo_config(config.clone()).build().unwrap_or_else(
        |error| {
            panic!("Migration initialization error: {:?}", error);
        },
    );
    let main_crate_ids = get_main_crate_ids_from_project(db, &config.clone().into());

    let components = find_components(db, &main_crate_ids);
    let systems = find_systems(db, &main_crate_ids);

    let mut table = Table::new();
    table.set_header(vec!["Name", "Type", "Address", "Deployed"]);

    for component in components {
        let address = get_contract_address(
            component.name.as_str(),
            config.clone().content.world.initializer_class_hash.unwrap_or_default(),
            config.content.world.address.unwrap_or_default(),
        );
        table.add_row(vec![
            component.name,
            "Component".into(),
            format!("{:#x}", address).into(),
            "false".into(),
        ]);
    }

    for system in systems {
        let address = get_contract_address(
            system.name.as_str(),
            config.clone().content.world.initializer_class_hash.unwrap_or_default(),
            config.content.world.address.unwrap_or_default(),
        );
        table.add_row(vec![
            system.name,
            "System".into(),
            format!("{:#x}", address).into(),
            "false".into(),
        ]);
    }

    println!("{table}");
}
