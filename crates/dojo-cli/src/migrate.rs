use std::env::current_dir;
use std::path::PathBuf;

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::get_main_crate_ids_from_project;
use clap::Args;
use dojo_lang::component::find_components;
use dojo_lang::db::DojoRootDatabaseBuilderEx;
use dojo_lang::system::find_systems;
use dojo_project::ProjectConfig;
use smol_str::SmolStr;

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

    let db = &mut RootDatabase::builder().with_dojo_config(config.clone()).build().unwrap_or_else(
        |error| {
            panic!("Migration initialization error: {:?}", error);
        },
    );
    let main_crate_ids = get_main_crate_ids_from_project(db, &config.into());

    let components = find_components(db, &main_crate_ids);
    let systems = find_systems(db, &main_crate_ids);

    println!(
        "components: {:#?}",
        components.iter().map(|p| p.name.clone()).collect::<Vec<SmolStr>>()
    );
    println!("systems: {:#?}", systems.iter().map(|p| p.name.clone()).collect::<Vec<SmolStr>>());
}
