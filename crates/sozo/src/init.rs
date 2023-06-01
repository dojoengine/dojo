use std::env::{current_dir, set_current_dir};
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::{fs, io};

use clap::Args;

#[derive(Args, Debug)]
pub struct InitArgs {
    #[clap(help = "Target directory")]
    path: Option<PathBuf>,

    #[clap(help = "Parse a full git url or a url path", default_value = "dojoengine/dojo-starter")]
    template: String,
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
    println!("\n\n ⛩️ ====== STARTING ====== ⛩️ \n");

    println!("Setting up project directory tree...");

    let template = args.template;
    let repo_url = if template.starts_with("https://") {
        template.to_string()
    } else {
        "https://github.com/".to_string() + &template
    };

    clone_repo(&repo_url, &target_dir)?;

    println!("✅ Project directory tree created successfully!");

    // Navigate to the newly cloned repo.
    let initial_dir = current_dir()?;
    set_current_dir(&target_dir)?;

    // Modify the git history.
    let git_output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output()?.stdout;
    let commit_hash = String::from_utf8(git_output)?;
    fs::remove_dir_all(".git")?;
    Command::new("git").arg("init").output()?;
    Command::new("git").args(["add", "--all"]).output()?;

    let commit_msg = format!("chore: init from {} at {}", repo_url, commit_hash.trim());
    Command::new("git").args(["commit", "-m", &commit_msg]).output()?;

    // Navigate back.
    set_current_dir(initial_dir)?;

    println!(
        "\n\n====== SETUP COMPLETE! ======\n\nTo start using your new Dojo project, try running: \
        \n\n`sozo build`\n"
    );

    println!("🎉🎉🎉 SUCCESS! Your project is now ready. Start building with ⛩️ Dojo! 🎉🎉🎉");

    Ok(())
}

fn clone_repo(url: &str, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        let entries = fs::read_dir(path)?.count();
        if entries > 0 {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                "Target directory is not empty",
            )));
        }
    }

    Command::new("git").args(&["clone", "--recursive", url, path.to_str().unwrap()]).output()?;

    Ok(())
}
