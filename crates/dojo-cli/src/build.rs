use std::env::current_dir;
use std::path::PathBuf;
use std::sync::Arc;

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::project::get_main_crate_ids_from_project;
use cairo_lang_compiler::CompilerConfig;
use cairo_lang_dojo::plugin::DojoPlugin;
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_project::ProjectConfig;
use cairo_lang_starknet::contract::find_contracts;
use cairo_lang_starknet::contract_class::compile_prepared_db;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use clap::Args;

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[clap(help = "Source directory")]
    path: Option<PathBuf>,
    /// The output file name (default: stdout).
    #[clap(help = "Output directory")]
    out_dir: Option<PathBuf>,
}

pub fn run(args: BuildArgs) {
    let source_dir = match args.path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                current_path
            }
        }
        None => current_dir().unwrap(),
    };
    let target_dir = match args.out_dir {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                base_path.push(path);
                base_path
            }
        }
        None => {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("target/ecs-sierra");
            path
        }
    };

    println!("\n\nWriting files to dir: {target_dir:#?}");

    let mut plugins = get_default_plugins();
    plugins.push(Arc::new(DojoPlugin {}));
    plugins.push(Arc::new(StarkNetPlugin {}));

    let mut config = ProjectConfig::from_directory(&source_dir).unwrap_or_else(|error| {
        panic!("Problem creating project config: {:?}", error);
    });

    config.corelib = Some(Directory("cairo/corelib".into()));

    let db = &mut RootDatabase::builder()
        .with_project_config(config.clone())
        .with_plugins(plugins)
        .build()
        .unwrap_or_else(|error| {
            panic!("Problem creating language database: {:?}", error);
        });
    let main_crate_ids = get_main_crate_ids_from_project(db, &config);

    // TODO: Error handling
    let contracts = find_contracts(db, &main_crate_ids);
    let contracts = contracts.iter().collect::<Vec<_>>();
    let classes = compile_prepared_db(db, &contracts, CompilerConfig::default());

    println!("COMPILE TEST: {:#?}", classes);
}
