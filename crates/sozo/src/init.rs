use std::env::current_dir;
use std::path::{Path, PathBuf};
use std::{fs, io};

use clap::Args;

#[derive(Args, Debug)]
pub struct InitArgs {
    #[clap(help = "Target directory")]
    path: Option<PathBuf>,
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn run(args: InitArgs) {
    let target_dir = match args.path {
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

    let template_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sozo-template");

    copy_dir_all(template_dir, target_dir).unwrap();

    println!("🗄 Creating project directory tree");
    println!("⛩️ Dojo project ready!");
    println!();
    println!("Try running: `dojo-test .`");
}
