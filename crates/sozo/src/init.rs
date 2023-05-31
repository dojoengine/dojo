use std::env::current_dir;
use std::error::Error;
use std::path::PathBuf;

use clap::Args;
use git2::Repository;

#[derive(Args, Debug)]
pub struct InitArgs {
    #[clap(help = "Target directory")]
    path: Option<PathBuf>,
}

pub fn run(args: InitArgs) -> Result<(), Box<dyn Error>> {
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
    println!("\n\n⛩️ ====== STARTING ======\n");

    println!("Setting up project directory tree...");

    let repo_url = "https://github.com/dojoengine/dojo-starter";
    clone_repo(repo_url, target_dir)?;

    println!("[✅ Project directory tree created successfully!");

    println!("\n\n====== SETUP COMPLETE! ======\n\nTo start using your new Dojo project, try running: \n\n\t`sozo build`\n");

    println!("🎉🎉🎉 SUCCESS! Your project is now ready. Enjoy working with Dojo! 🎉🎉🎉");

    Ok(())
}

fn clone_repo(url: &str, path: PathBuf) -> Result<(), Box<dyn Error>> {
    let _repo = Repository::clone(url, path)?;

    Ok(())
}
