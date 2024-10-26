use std::fs;

use anyhow::{Context, Result};
use camino::Utf8Path;
use scarb::core::Workspace;
use scarb::flock::Filesystem;

/// Handy enum for selecting the current profile or all profiles.
#[derive(Debug)]
pub enum ProfileSpec {
    WorkspaceCurrent,
    All,
}

/// Extension trait for the [`Filesystem`] type.
pub trait FilesystemExt {
    /// Returns a new Filesystem with the given subdirectories.
    ///
    /// This is a helper function since flock [`Filesystem`] only has a child method.
    fn children(&self, sub_dirs: &[impl AsRef<Utf8Path>]) -> Filesystem;

    /// Lists all the files in the filesystem root, not recursively.
    fn list_files(&self) -> Result<Vec<String>>;
}

impl FilesystemExt for Filesystem {
    fn children(&self, sub_dirs: &[impl AsRef<Utf8Path>]) -> Self {
        if sub_dirs.is_empty() {
            return self.clone();
        }

        let mut result = self.clone();

        for sub_dir in sub_dirs {
            result = result.child(sub_dir);
        }

        result
    }

    fn list_files(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();

        let path = self.to_string();

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }

        Ok(files)
    }
}

/// Extension trait for the [`Workspace`] type.
pub trait WorkspaceExt {
    /// Returns the target directory for the current profile.
    fn target_dir_profile(&self) -> Filesystem;
    /// Checks if the current profile is valid for the workspace.
    fn profile_check(&self) -> Result<()>;
    /// Cleans the target directory for the current profile.
    fn clean_dir_profile(&self) -> Result<()>;
    /// Cleans the target directory for all profiles.
    fn clean_dir_all_profiles(&self) -> Result<()>;
}

impl WorkspaceExt for Workspace<'_> {
    fn target_dir_profile(&self) -> Filesystem {
        self.target_dir()
            .child(self.current_profile().expect("Current profile always exists").as_str())
    }

    fn profile_check(&self) -> Result<()> {
        if let Err(e) = self.current_profile() {
            if e.to_string().contains("has no profile") {
                // Extract the profile name from the error message
                if let Some(profile_name) = e.to_string().split('`').nth(3) {
                    anyhow::bail!(
                        "Profile '{}' not found in workspace. Consider adding [profile.{}] to \
                         your Scarb.toml to declare the profile.",
                        profile_name,
                        profile_name
                    );
                }
            }
            anyhow::bail!("Profile check failed: {}", e);
        }

        Ok(())
    }

    fn clean_dir_profile(&self) -> Result<()> {
        let target_dir = self.target_dir_profile();
        fs::remove_dir_all(target_dir.to_string()).context("failed to clean generated artifacts")
    }

    fn clean_dir_all_profiles(&self) -> Result<()> {
        let target_dir = self.target_dir();
        fs::remove_dir_all(target_dir.to_string()).context("failed to clean generated artifacts")
    }
}
