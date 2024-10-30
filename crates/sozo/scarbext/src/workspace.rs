use std::fs;
use std::ops::DerefMut;

use anyhow::Result;
use dojo_world::config::ProfileConfig;
use dojo_world::local::WorldLocal;
use scarb::core::Workspace;
use scarb::flock::Filesystem;
use serde::Serialize;

use crate::filesystem::FilesystemExt;

/// Extension trait for the [`Workspace`] type.
pub trait WorkspaceExt {
    /// Returns the target directory for the current profile.
    fn target_dir_profile(&self) -> Filesystem;
    /// Checks if the current profile is valid for the workspace.
    fn profile_check(&self) -> Result<()>;
    /// Cleans the target directory for the current profile.
    fn clean_dir_profile(&self);
    /// Cleans the target directory for all profiles.
    fn clean_dir_all_profiles(&self);
    /// Checks if the current profile has generated artifacts.
    fn ensure_profile_artifacts(&self) -> Result<()>;
    /// Loads the profile config for the current profile.
    fn load_profile_config(&self) -> Result<ProfileConfig>;
    /// Loads the local world from the workspace configuration.
    fn load_world_local(&self) -> Result<WorldLocal>;
    /// Writes the manifest for the current profile.
    fn write_manifest_profile(&self, manifest: impl Serialize) -> Result<()>;
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

    fn clean_dir_profile(&self) {
        let target_dir = self.target_dir_profile();
        // Ignore errors since the directory might not exist.
        let _ = fs::remove_dir_all(target_dir.to_string());
    }

    fn clean_dir_all_profiles(&self) {
        let target_dir = self.target_dir();
        // Ignore errors since the directory might not exist.
        let _ = fs::remove_dir_all(target_dir.to_string());
    }

    fn ensure_profile_artifacts(&self) -> Result<()> {
        let profile_name = self.current_profile()?.to_string();

        if !self.target_dir_profile().exists() || self.target_dir_profile().list_files()?.is_empty()
        {
            if profile_name == "dev" {
                anyhow::bail!(
                    "No artifacts generated for the 'dev' profile. Run `sozo build` to generate \
                     them since it's the default profile."
                );
            } else {
                anyhow::bail!(
                    "Target directory for profile '{}' does not exist or is empty, run `sozo \
                     build --profile {}` to generate it.",
                    profile_name,
                    profile_name
                );
            }
        }

        Ok(())
    }

    fn load_profile_config(&self) -> Result<ProfileConfig> {
        // Safe to unwrap since manifest is a file.
        let manifest_dir = self.manifest_path().parent().unwrap().to_path_buf();
        let profile_str =
            self.current_profile().expect("Scarb profile expected to be defined.").to_string();

        let dev_config_path = manifest_dir.join("dojo_dev.toml");
        let config_path = manifest_dir.join(format!("dojo_{}.toml", &profile_str));

        if !dev_config_path.exists() {
            return Err(anyhow::anyhow!(
                "Profile configuration file not found for profile `{}`. Expected at {}.",
                &profile_str,
                dev_config_path
            ));
        }

        // If the profile file is not found, default to `dev.toml` file that must exist.
        let config_path = if !config_path.exists() { dev_config_path } else { config_path };

        let content = fs::read_to_string(&config_path)?;
        let config: ProfileConfig = toml::from_str(&content)?;

        Ok(config)
    }

    fn load_world_local(&self) -> Result<WorldLocal> {
        WorldLocal::from_directory(
            self.target_dir_profile().to_string(),
            self.load_profile_config()?,
        )
    }

    fn write_manifest_profile(&self, manifest: impl Serialize) -> Result<()> {
        let profile_name = self.current_profile()?.to_string();
        let manifest_name = format!("manifest_{}.json", &profile_name);

        let manifest_dir = self.manifest_path().parent().unwrap();
        let manifest_dir = Filesystem::new(manifest_dir.into());

        let mut file =
            manifest_dir.create_rw(manifest_name, "Dojo manifest file", self.config())?;

        Ok(serde_json::to_writer_pretty(file.deref_mut(), &manifest)?)
    }
}
