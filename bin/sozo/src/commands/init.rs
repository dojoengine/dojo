use std::env::{current_dir, set_current_dir};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use sozo_ui::SozoUi;

use anyhow::{Context, Result, ensure};
use clap::Args;
use tracing::trace;

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(help = "Target directory")]
    path: Option<PathBuf>,

    #[arg(
        long,
        help = "Parse a full git url or a url path",
        default_value = "dojoengine/dojo-starter"
    )]
    template: String,

    #[arg(long, help = "Initialize a new Git repository")]
    git: bool,
}

impl InitArgs {
    pub fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);
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
        trace!(?target_dir);

        if target_dir.exists() {
            ensure!(
                fs::read_dir(&target_dir)?.next().is_none(),
                io::Error::other("Target directory is not empty")
            );
        }

        ui.print("\n\n ⛩️ ====== STARTING ====== ⛩️ \n");
        ui.print("Setting up project directory tree...");

        let template = self.template;
        let repo_url = if template.starts_with("https://") {
            template
        } else {
            "https://github.com/".to_string() + &template
        };

        let sozo_version = match get_sozo_version() {
            Ok(version) => version,
            Err(e) => return Err(e.context("Failed to get Sozo version")),
        };

        trace!(repo_url = repo_url, sozo_version = sozo_version);

        clone_repo(&repo_url, &target_dir, &sozo_version, ui)?;

        // Navigate to the newly cloned repo.
        let initial_dir = current_dir()?;
        set_current_dir(&target_dir)?;

        // Modify the git history.
        modify_git_history(&repo_url, self.git)?;

        ui.print("\n🎉 Successfully created a new ⛩️ Dojo project!");

        // Navigate back.
        set_current_dir(initial_dir)?;

        ui.print(
            "\n====== SETUP COMPLETE! ======\n\n\nTo start using your new project, try running: \
             `sozo build`",
        );

        trace!("Project initialization completed.");

        Ok(())
    }
}

fn get_sozo_version() -> Result<String> {
    let output = Command::new("sozo")
        .arg("--version")
        .output()
        .context("Failed to execute `sozo --version` command")?;

    let version_string = String::from_utf8(output.stdout)
        .context("Failed to parse `sozo --version` output as UTF-8")?;

    if let Some(first_line) = version_string.lines().next() {
        if let Some(version) = first_line.split_whitespace().nth(1) {
            return Ok(version.to_string());
        }
    }

    Err(anyhow::anyhow!("Failed to parse sozo version"))
}

fn check_tag_exists(url: &str, version: &str) -> Result<bool> {
    trace!(url = url, version = version, "Checking tag.");
    let output = Command::new("git").args(["ls-remote", "--tags", url]).output()?;

    let output_str = String::from_utf8(output.stdout)?;
    let tag_exists = output_str.contains(&format!("refs/tags/v{}", version));

    Ok(tag_exists)
}

fn clone_repo(url: &str, path: &Path, version: &str, ui: &SozoUi) -> Result<()> {
    // Check if the version tag exists in the repository
    let tag_exists = check_tag_exists(url, version)?;

    if tag_exists {
        ui.print(format!("Cloning project template from {}...", url));
        Command::new("git")
            .args([
                "clone",
                "--branch",
                &format!("v{}", version),
                "--single-branch",
                "--recursive",
                url,
                path.to_str().unwrap(),
            ])
            .output()?;
    } else {
        ui.warn(
            "Couldn't find template for your current sozo version. Getting the latest version
            instead.",
        );
        Command::new("git").args(["clone", "--recursive", url, path.to_str().unwrap()]).output()?;
    }

    trace!("Repository cloned successfully.");
    Ok(())
}

fn modify_git_history(url: &str, init_git: bool) -> Result<()> {
    trace!("Modifying Git history.");
    let git_output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output()?.stdout;
    let commit_hash = String::from_utf8(git_output)?;
    trace!(commit_hash = commit_hash.trim());

    fs::remove_dir_all(".git")?;
    if Path::new(".github").exists() {
        fs::remove_dir_all(".github")?;
    }

    if init_git {
        Command::new("git").arg("init").output()?;
        Command::new("git").args(["add", "--all"]).output()?;

        let commit_msg = format!("chore: init from {} at {}", url, commit_hash.trim());
        Command::new("git").args(["commit", "-m", &commit_msg]).output()?;
    }

    trace!("Git history modified.");
    Ok(())
}
