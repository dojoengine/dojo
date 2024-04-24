use std::env::{current_dir, set_current_dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use anyhow::{ensure, Result};
use clap::Args;
use scarb::core::Config;
use tracing::trace;

pub(crate) const LOG_TARGET: &str = "sozo::cli::commands::init";

#[derive(Debug, Args)]
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
                    trace!(target: LOG_TARGET, ?path);
                    path
                } else {
                    let mut current_path = current_dir().unwrap();
                    current_path.push(path);
                    trace!(target: LOG_TARGET, ?current_path);
                    current_path
                }
            }
            None => {
                let dir = current_dir().unwrap();
                trace!(target: LOG_TARGET, ?dir);
                dir
            }
        };

        if target_dir.exists() {
            ensure!(
                fs::read_dir(&target_dir)?.next().is_none(),
                io::Error::new(io::ErrorKind::Other, "Target directory is not empty",)
            );
            trace!(target: LOG_TARGET, "Target directory is empty.");
        } else {
            trace!(target: LOG_TARGET, "Target directory does not exist.");
        }

        config.ui().print("\n\n ⛩️ ====== STARTING ====== ⛩️ \n");
        config.ui().print("Setting up project directory tree...");

        let template = self.template;
        let repo_url = if template.starts_with("https://") {
            template
        } else {
            let url = "https://github.com/".to_string() + &template;
            trace!(target: LOG_TARGET, url, "Constructed Git URL.");
            url
        };

        clone_repo(&repo_url, &target_dir, config)?;

        // Navigate to the newly cloned repo.
        let initial_dir = current_dir()?;
        set_current_dir(&target_dir)?;
        trace!(target: LOG_TARGET, ?target_dir);

        // Modify the git history.
        modify_git_history(&repo_url)?;

        config.ui().print("\n🎉 Successfully created a new ⛩️ Dojo project!");

        // Navigate back.
        trace!(target: LOG_TARGET, ?initial_dir, "Returned to initial directory.");
        set_current_dir(initial_dir)?;

        config.ui().print(
            "\n====== SETUP COMPLETE! ======\n\n\nTo start using your new project, try running: \
             `sozo build`",
        );

        trace!(target: LOG_TARGET, "Project initialization completed.");

        Ok(())
    }
}

fn clone_repo(url: &str, path: &Path, config: &Config) -> Result<()> {
    config.ui().print(format!("Cloning project template from {}...", url));
    Command::new("git").args(["clone", "--recursive", url, path.to_str().unwrap()]).output()?;
    trace!(target: LOG_TARGET, "Repository cloned successfully.");
    Ok(())
}

fn modify_git_history(url: &str) -> Result<()> {
    trace!(target: LOG_TARGET, "Modifying Git history.");
    let git_output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output()?.stdout;
    let commit_hash = String::from_utf8(git_output)?;
    trace!(
        target: LOG_TARGET,
        commit_hash=commit_hash.trim()
    );

    fs::remove_dir_all(".git")?;

    Command::new("git").arg("init").output()?;
    Command::new("git").args(["add", "--all"]).output()?;

    let commit_msg = format!("chore: init from {} at {}", url, commit_hash.trim());
    Command::new("git").args(["commit", "-m", &commit_msg]).output()?;

    trace!(target: LOG_TARGET, "Git history modified.");
    Ok(())
}