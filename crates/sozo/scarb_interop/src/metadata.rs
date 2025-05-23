//! Extension trait for the [`scarb-metadata::Metadata`] type.
//!
//! This is mostly to organize the work around the profiles,
//! which is crucial for the Dojo configuration relying heavily on them.
use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_world::config::ProfileConfig;
use dojo_world::diff::Manifest;
use dojo_world::local::WorldLocal;
use scarb_metadata::{DepKind, Metadata, MetadataCommand, MetadataCommandError};
use serde::Serialize;

use crate::fsx;

#[derive(Debug, PartialEq)]
pub enum TestRunner {
    NoTestRunner,
    CairoTestRunner,
    SnfTestRunner,
}

const CAIRO_TEST_RUNNER_NAME: &str = "cairo_test";
const SNF_TEST_RUNNER_NAME: &str = "snforge_std";

impl From<&String> for TestRunner {
    fn from(value: &String) -> Self {
        match value.as_str() {
            CAIRO_TEST_RUNNER_NAME => Self::CairoTestRunner,
            SNF_TEST_RUNNER_NAME => Self::SnfTestRunner,
            _ => Self::NoTestRunner,
        }
    }
}

/// Extension trait for the [`Metadata`] type.
pub trait MetadataDojoExt {
    /// Returns the workspace package name. If it's a virtual workspace, there's no package name and
    /// returns an error. In the case of Dojo, it's never expected to be a virtual workspace.
    fn workspace_package_name(&self) -> Result<String>;
    /// Returns the target directory root for the workspace.
    fn target_dir_root(&self) -> Utf8PathBuf;
    /// Returns the target directory for the current profile.
    fn target_dir_profile(&self) -> Utf8PathBuf;
    /// Cleans the target directory for the current profile.
    fn clean_dir_profile(&self);
    /// Cleans the target directory for all profiles.
    fn clean_dir_all_profiles(&self);
    /// Checks if the current profile has generated artifacts.
    fn ensure_profile_artifacts(&self) -> Result<()>;
    /// Loads the Dojo profile config for the current profile.
    fn load_dojo_profile_config(&self) -> Result<ProfileConfig>;
    /// Loads the local world from the workspace configuration.
    fn load_dojo_world_local(&self) -> Result<WorldLocal>;
    /// Writes the Dojo manifest for the current profile.
    fn write_dojo_manifest_profile(&self, manifest: impl Serialize) -> Result<()>;
    /// Reads the Dojo manifest for the current profile.
    fn read_dojo_manifest_profile(&self) -> Result<Option<Manifest>>;
    /// Returns the dojo manifest path for the current profile.
    fn dojo_manifest_path_profile(&self) -> Utf8PathBuf;
    /// Indicates which test runner is used in the project
    fn test_runner(&self) -> Result<TestRunner>;
    /// load metadata
    fn load(manifest_path: &Utf8PathBuf, profile: &str, offline: bool) -> Result<Metadata>;
}

impl MetadataDojoExt for Metadata {
    fn workspace_package_name(&self) -> Result<String> {
        // Read the toml file at the workspace root and check if the [package] table in toml
        // contains a `name` field. If we don't have [package] but [workspace.package] we know it's
        // a virtual workspace.
        let toml_path = &self.workspace.manifest_path;
        let toml_content = fsx::read_to_string(toml_path)?;
        let toml: toml::Value = toml::from_str(&toml_content)?;

        let package_name = toml.get("package").and_then(|v| v.get("name")).and_then(|v| v.as_str());

        match package_name {
            Some(name) => Ok(name.to_string()),
            None => anyhow::bail!(
                "No package name found in {}. Sozo currently supports only non-virtual workspaces.",
                toml_path
            ),
        }
    }
    fn target_dir_root(&self) -> Utf8PathBuf {
        if let Some(target_dir) = &self.target_dir {
            target_dir.clone()
        } else {
            self.workspace.root.clone().join("target")
        }
    }

    fn target_dir_profile(&self) -> Utf8PathBuf {
        self.target_dir_root().join(self.current_profile.as_str())
    }

    fn clean_dir_profile(&self) {
        let target_dir = self.target_dir_profile();
        // Ignore errors since the directory might not exist.
        let _ = fsx::remove_dir_all(&target_dir);
    }

    fn clean_dir_all_profiles(&self) {
        let target_dir = self.target_dir_root();
        // Ignore errors since the directory might not exist.
        let _ = fsx::remove_dir_all(&target_dir);
    }

    fn ensure_profile_artifacts(&self) -> Result<()> {
        let profile_name = self.current_profile.as_str();

        if !self.target_dir_profile().exists()
            || fsx::list_files(self.target_dir_profile())?.is_empty()
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

    fn load_dojo_profile_config(&self) -> Result<ProfileConfig> {
        // Safe to unwrap since manifest is a file.
        let manifest_dir = &self.workspace.root;
        let profile_str = self.current_profile.as_str();

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

        let content = fsx::read_to_string(&config_path)?;
        let config: ProfileConfig = toml::from_str(&content)?;

        config.validate()?;

        Ok(config)
    }

    fn load_dojo_world_local(&self) -> Result<WorldLocal> {
        WorldLocal::from_directory(self.target_dir_profile(), self.load_dojo_profile_config()?)
    }

    fn write_dojo_manifest_profile(&self, manifest: impl Serialize) -> Result<()> {
        let profile_name = self.current_profile.as_str();
        let manifest_name = format!("manifest_{}.json", &profile_name);

        let manifest_dir = &self.workspace.root;
        // TODO: we may want here to use some global lock
        // to ensure the file is written without any race condition.
        // Re-use `flock` from `scarb` to do so.

        let file = fsx::create(manifest_dir.join(manifest_name))?;

        Ok(serde_json::to_writer_pretty(file, &manifest)?)
    }

    fn read_dojo_manifest_profile(&self) -> Result<Option<Manifest>> {
        let profile_name = self.current_profile.as_str();
        let manifest_name = format!("manifest_{}.json", &profile_name);

        let manifest_dir = &self.workspace.root;
        let manifest_path = manifest_dir.join(manifest_name);

        if !manifest_path.exists() {
            return Ok(None);
        }

        let content = fsx::read_to_string(manifest_path)?;

        Ok(Some(serde_json::from_str(&content)?))
    }

    fn dojo_manifest_path_profile(&self) -> Utf8PathBuf {
        let profile_name = self.current_profile.as_str();
        let manifest_name = format!("manifest_{}.json", &profile_name);

        self.workspace.root.join(manifest_name)
    }

    fn test_runner(&self) -> Result<TestRunner> {
        let package_name = self.workspace_package_name()?;
        let dev_dependencies = self
            .packages
            .iter()
            .filter(|p| p.name == package_name)
            .flat_map(|p| {
                p.dependencies
                    .iter()
                    .filter(|d| {
                        d.kind.clone().is_some_and(|kind| kind == DepKind::Dev)
                            && (d.name == CAIRO_TEST_RUNNER_NAME || d.name == SNF_TEST_RUNNER_NAME)
                    })
                    .map(|d| d.name.clone())
            })
            .collect::<Vec<_>>();

        if dev_dependencies.is_empty() {
            return Ok(TestRunner::NoTestRunner);
        }

        Ok(TestRunner::from(dev_dependencies.first().unwrap()))
    }

    fn load(manifest_path: &Utf8PathBuf, profile: &str, offline: bool) -> Result<Self> {
        let mut metadata = MetadataCommand::new();
        metadata.manifest_path(manifest_path);
        metadata.profile(profile);

        if offline {
            metadata.no_deps();
        }

        metadata.exec().map_err(|err| anyhow::anyhow!(err.format_error_message(manifest_path)))
    }
}

/// Extension trait for the [`MetadataCommandError`] type to provide
/// more context about the error.
pub trait MetadataErrorExt {
    fn format_error_message(&self, manifest_path: &Utf8PathBuf) -> String;
}

impl MetadataErrorExt for MetadataCommandError {
    fn format_error_message(&self, manifest_path: &Utf8PathBuf) -> String {
        match self {
            Self::ScarbError { stdout, stderr } => {
                if stdout.contains("has no profile") {
                    let profile_name =
                        stdout.split("has no profile").nth(1).unwrap_or("").trim().replace("`", "");
                    format!(
                        "The profile '{}' does not exist. Consider adding [profile.{}] to `{}` to \
                         declare the profile.",
                        profile_name, profile_name, manifest_path
                    )
                } else {
                    format!(
                        "Error while executing scarb metadata \
                         command:\nstdout:\n{}\nstderr:\n{}\nPlease verify that the `$SCARB` \
                         environment variable is set correctly and match the scarb executable \
                         version.\n$SCARB={}",
                        stdout,
                        stderr,
                        std::env::var("SCARB").unwrap_or("NOT SET".to_string())
                    )
                }
            }
            _ => format!("Error while executing scarb metadata command:\n{}", self),
        }
    }
}
