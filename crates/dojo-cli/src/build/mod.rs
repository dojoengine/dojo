use std::env::current_dir;
use std::fs;
use std::path::PathBuf;

use cairo_lang_dojo::build::{build_corelib, reset_corelib};
use cairo_lang_dojo::compiler::compile_dojo_project_at_path;
use clap::Args;
use pathdiff::diff_paths;

use crate::utils::get_cairo_files_in_path;

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

    let cairo_files = get_cairo_files_in_path(&source_dir);

    let mut files_added: Vec<String> = vec![];

    println!("\n\nWriting files to dir: {target_dir:#?}");
    reset_corelib();
    for cairo_file in cairo_files.iter() {
        build_corelib(cairo_file.clone());
        let program_result = compile_dojo_project_at_path(cairo_file);
        let path_relative = diff_paths(cairo_file, &source_dir).unwrap();
        files_added.push(path_relative.to_str().unwrap().into());
        match program_result {
            Ok(program) => {
                let mut path_absolute = target_dir.clone();

                let filename = cairo_file.iter().last().unwrap();
                path_absolute.push(filename);
                path_absolute.set_extension("sierra");
                println!("Writing file: {path_absolute:#?}");

                let mut dir = path_absolute.clone();
                let _r = dir.pop();
                let _r = fs::create_dir_all(dir);
                let _r = fs::write(path_absolute, format!("{}", program));

                files_added.push("Successfully added".into());
            }
            Err(err) => {
                files_added.push(path_relative.to_str().unwrap().into());
                files_added.push(format!("{err:#?}"));
            }
        }
    }

    for (i, status) in files_added.iter().enumerate() {
        if i % 2 == 0 {
            println!(); // New line
        }
        println!("{status}");
    }
}
