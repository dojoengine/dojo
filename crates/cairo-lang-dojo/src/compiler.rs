use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_compiler::project::setup_project;
use cairo_lang_compiler::CompilerConfig;
use cairo_lang_compiler::{db::RootDatabase, project::ProjectError};
use cairo_lang_diagnostics::ToOption;
use cairo_lang_filesystem::{db::FilesGroup, ids::CrateLongId};
use cairo_lang_project::ProjectConfig;
use cairo_lang_sierra::program::Program;
use cairo_lang_sierra_generator::db::SierraGenGroup;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract::find_contracts;
use cairo_lang_starknet::contract_class::{compile_prepared_db, ContractClass};
use clap::Parser;

use crate::db::DojoRootDatabaseBuilderEx;

/// Command line args parser.
/// Exits with 0/1 if the input is formatted correctly/incorrectly.
#[derive(Parser, Debug)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// The file to compile
    path: String,
    /// The output file name (default: stdout).
    output: Option<String>,
    /// Replaces sierra ids with human readable ones.
    #[arg(short, long, default_value_t = false)]
    replace_ids: bool,
}

pub fn compile_dojo_project_at_path(path: &Path) -> anyhow::Result<Vec<ContractClass>> {
    let db = &mut RootDatabase::builder().detect_corelib().with_dojo_and_starknet().build()?;

    // let path = config.base_path;

    let project = setup_project(db, path)?;
    // let crate_name = file_dir.to_str().ok_or_else(bad_path_err)?;
    // let main_crate_ids = vec![db.intern_crate(CrateLongId(crate_name.into()))];

    // // if DiagnosticsReporter::stderr().check(db) {
    // //     anyhow::bail!("failed to compile: {:?}", path);
    // // }

    println!("COMPILE TEST: {:?}", project);

    // let contracts = find_contracts(db, &main_crate_ids);
    // let contracts = contracts.iter().collect::<Vec<_>>();

    // let classes = compile_prepared_db(db, &contracts, CompilerConfig::default());

    // let sierra_program = db
    //     .get_sierra_program(main_crate_ids)
    //     .to_option()
    //     .context("Compilation failed without any diagnostics")
    //     .unwrap();

    Ok(vec![])
}

// #[test]
// fn test_compile() {
//     use std::path::PathBuf;

//     let mut test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
//     test_path.push("src/cairo_level_tests");

//     let program = compile_dojo_project_at_path(&test_path);
//     println!("COMPILE TEST: {program:#?}");
//     todo!("Add asserts for comile tests.");
// }
