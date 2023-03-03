use std::env::{self, current_dir};
use std::path::PathBuf;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_lang::compiler::DojoCompiler;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,
    /// The output file name (default: stdout).
    #[clap(help = "Output directory")]
    out_dir: Option<PathBuf>,
}

pub fn run(args: BuildArgs) -> Result<()> {
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

    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let config = Config::builder(source_dir)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        std::process::exit(1);
    });
    ops::compile(&ws)
}
