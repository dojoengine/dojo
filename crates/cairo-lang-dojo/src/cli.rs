use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use cairo_lang_dojo::build::{build_corelib, reset_corelib};
use cairo_lang_dojo::compiler::compile_dojo_project_at_path;
use clap::Parser;

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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let path = &PathBuf::from(args.path);
    let corelib_path = PathBuf::from("corelib");
    reset_corelib(corelib_path);
    build_corelib(path.clone());

    let sierra_program = compile_dojo_project_at_path(path)?;

    match args.output {
        Some(path) => {
            fs::write(path, format!("{}", sierra_program)).context("Failed to write output.")?
        }
        None => println!("{}", sierra_program),
    }

    Ok(())
}
