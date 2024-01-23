use std::env::{current_dir, set_current_dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use anyhow::{ensure, Result};
use clap::Args;
use scarb::core::Config;

#[derive(Args, Debug)]
pub struct InitArgs {
    #[arg(help = "Target directory")]
    path: Option<PathBuf>,

    #[arg(help = "Parse a full git url or a url path", default_value = "dojoengine/dojo-starter")]
    template: String,
}

impl InitArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let target_dir = match self.path {
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

        if target_dir.exists() {
            ensure!(
                fs::read_dir(&target_dir)?.next().is_none(),
                io::Error::new(io::ErrorKind::Other, "Target directory is not empty",)
            );
        }

        config.ui().print("\n\n ⛩️ ====== STARTING ====== ⛩️ \n");
        config.ui().print("Setting up project directory tree...");

        let template = self.template;
        let repo_url = if template.starts_with("https://") {
            template
        } else {
            "https://github.com/".to_string() + &template
        };

        clone_repo(&repo_url, &target_dir, config)?;

        // Navigate to the newly cloned repo.
        let initial_dir = current_dir()?;
        set_current_dir(&target_dir)?;

        // Modify the git history.
        modify_git_history(&repo_url)?;

        config.ui().print("\n🎉 Successfully created a new ⛩️ Dojo project!");

        // Navigate back.
        set_current_dir(initial_dir)?;

        config.ui().print(
            "\n====== SETUP COMPLETE! ======\n\n\nTo start using your new project, try running: \
             `sozo build`",
        );

        Ok(())
    }
}

fn clone_repo(url: &str, path: &Path, config: &Config) -> Result<()> {
    config.ui().print(format!("Cloning project template from {}...", url));
    Command::new("git").args(["clone", "--recursive", url, path.to_str().unwrap()]).output()?;
    Ok(())
}

fn modify_git_history(url: &str) -> Result<()> {
    let git_output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output()?.stdout;
    let commit_hash = String::from_utf8(git_output)?;

    fs::remove_dir_all(".git")?;

    Command::new("git").arg("init").output()?;
    Command::new("git").args(["add", "--all"]).output()?;

    let commit_msg = format!("chore: init from {} at {}", url, commit_hash.trim());
    Command::new("git").args(["commit", "-m", &commit_msg]).output()?;

    Ok(())
}
