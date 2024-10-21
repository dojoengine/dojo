use std::fs;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Args;
use dojo_world::manifest::{BASE_DIR, MANIFESTS_DIR};
use scarb::core::Config;
use scarb::ops;
use tracing::trace;

#[derive(Debug, Args)]
pub struct CleanArgs {
    #[arg(long)]
    #[arg(help = "Removes all the generated files, including scarb artifacts and ALL the \
                  manifests files.")]
    pub full: bool,

    #[arg(long)]
    #[arg(help = "Clean all profiles.")]
    pub all_profiles: bool,
}

impl CleanArgs {
    /// Cleans the manifests and abis files that are generated at build time.
    ///
    /// # Arguments
    ///
    /// * `profile_dir` - The directory where the profile files are located.
    pub fn clean_manifests(profile_dir: &Utf8PathBuf) -> Result<()> {
        trace!(?profile_dir, "Cleaning manifests.");
        let dirs = vec![profile_dir.join(BASE_DIR)];

        for d in dirs {
            if d.exists() {
                trace!(directory=?d, "Removing directory.");
                fs::remove_dir_all(d)?;
            }
        }

        Ok(())
    }

    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        trace!(ws=?ws, "Workspace read successfully.");

        let profile_names = if self.all_profiles {
            ws.profile_names()
        } else {
            vec![
                ws.current_profile().expect("Scarb profile is expected at this point.").to_string(),
            ]
        };

        for profile_name in profile_names {
            // Manifest path is always a file, we can unwrap safely to get the
            // parent folder.
            let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();

            // By default, this command cleans the build manifests and scarb artifacts.
            trace!("Cleaning Scarb artifacts and build manifests.");

            {
                // copied from scarb::ops::clean since scarb cleans build file of all the profiles
                // we only want to clean build files for specified profile
                //
                // cleaning build files for all profiles would create inconsistency with the
                // manifest files in `manifests` directory
                let ws = ops::read_workspace(config.manifest_path(), config)?;
                let path = ws.target_dir().path_unchecked().join(&profile_name);
                if path.exists() {
                    fs::remove_dir_all(path).context("failed to clean generated artifacts")?;
                }
            }

            let profile_dir = manifest_dir.join(MANIFESTS_DIR).join(&profile_name);

            Self::clean_manifests(&profile_dir)?;

            if self.full && profile_dir.exists() {
                trace!(?profile_dir, "Removing entire profile directory.");
                fs::remove_dir_all(profile_dir)?;
            }
        }

        Ok(())
    }
}

// these tests assume `example/spawn-and-move` is build for `dev` and `release` profile,
// which are normally built by the `build.rs` of `dojo-test-utils`.
#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler::CompilerTestSetup;
    use dojo_world::manifest::DEPLOYMENT_DIR;
    use dojo_world::metadata::ABIS_DIR;
    use scarb::compiler::Profile;

    use super::*;

    #[test]
    fn default_clean_works() {
        let setup = CompilerTestSetup::from_examples("../../crates/dojo/core", "../../examples/");
        let config = setup.build_test_config("spawn-and-move", Profile::DEV);

        let temp_project_dir = config.manifest_path().parent().unwrap().to_path_buf();

        println!("temp_project_dir: {:?}", temp_project_dir);

        let clean_cmd = CleanArgs { full: false, all_profiles: false };
        clean_cmd.run(&config).unwrap();

        let dev_profile_name = "dev";
        let release_profile_name = "release";

        let target_dev_dir = temp_project_dir.join("target").join(dev_profile_name);
        let target_release_dir = temp_project_dir.join("target").join(release_profile_name);

        let dev_manifests_dir = temp_project_dir.join("manifests").join(dev_profile_name);
        let release_manifests_dir = temp_project_dir.join("manifests").join(release_profile_name);

        let dev_manifests_base_dir = dev_manifests_dir.join("base");
        let dev_manifests_abis_base_dir = dev_manifests_dir.join("base").join("abis");
        let release_manifests_base_dir = release_manifests_dir.join("base");
        let release_manifests_abis_base_dir = release_manifests_dir.join("base").join("abis");

        let dev_manifests_depl_dir = dev_manifests_dir.join("deployment");
        let dev_manifests_abis_depl_dir = dev_manifests_depl_dir.join("abis");
        let dev_manifest_toml = dev_manifests_depl_dir.join("manifest").with_extension("toml");
        let dev_manifest_json = dev_manifests_depl_dir.join("manifest").with_extension("json");

        assert!(fs::read_dir(target_dev_dir).is_err(), "Expected 'target/dev' to be empty");
        assert!(
            fs::read_dir(target_release_dir).is_ok(),
            "Expected 'target/release' to be present"
        );

        assert!(
            fs::read_dir(dev_manifests_base_dir).is_err(),
            "Expected 'manifests/dev/base' to be empty"
        );
        assert!(
            fs::read_dir(dev_manifests_abis_base_dir).is_err(),
            "Expected 'manifests/dev/base/abis' to be empty"
        );
        assert!(
            fs::read_dir(&dev_manifests_abis_depl_dir).is_ok(),
            "Expected 'manifests/dev/deployment/abis' to be non empty"
        );

        // we expect release profile to be not affected
        assert!(
            fs::read_dir(release_manifests_base_dir).is_ok(),
            "Expected 'manifests/release/base' to be non empty"
        );
        assert!(
            fs::read_dir(release_manifests_abis_base_dir).is_ok(),
            "Expected 'manifests/release/base/abis' to be non empty"
        );

        assert!(dev_manifest_toml.exists(), "Expected 'manifest.toml' to exist");
        assert!(dev_manifest_json.exists(), "Expected 'manifest.json' to exist");

        let clean_cmd = CleanArgs { full: true, all_profiles: false };
        clean_cmd.run(&config).unwrap();

        assert!(
            fs::read_dir(&dev_manifests_abis_depl_dir).is_err(),
            "Expected 'manifests/dev/deployment/abis' to be empty"
        );
        assert!(!dev_manifest_toml.exists(), "Expected 'manifest.toml' to not exist");
        assert!(!dev_manifest_json.exists(), "Expected 'manifest.json' to not exist");
    }

    #[test]
    fn all_profile_clean_works() {
        let setup = CompilerTestSetup::from_examples("../../crates/dojo/core", "../../examples/");

        let config = setup.build_test_config("spawn-and-move", Profile::DEV);

        let temp_project_dir = config.manifest_path().parent().unwrap().to_path_buf();

        let clean_cmd = CleanArgs { full: false, all_profiles: true };
        clean_cmd.run(&config).unwrap();

        let dev_profile_name = "dev";
        let release_profile_name = "release";

        let target_dev_dir = temp_project_dir.join("target").join(dev_profile_name);
        let target_release_dir = temp_project_dir.join("target").join(release_profile_name);

        let dev_manifests_dir = temp_project_dir.join(MANIFESTS_DIR).join(dev_profile_name);
        let release_manifests_dir = temp_project_dir.join(MANIFESTS_DIR).join(release_profile_name);

        let dev_manifests_base_dir = dev_manifests_dir.join(BASE_DIR);
        let dev_manifests_abis_base_dir = dev_manifests_base_dir.join(ABIS_DIR);
        let release_manifests_base_dir = release_manifests_dir.join(BASE_DIR);
        let release_manifests_abis_base_dir = release_manifests_base_dir.join(ABIS_DIR);

        let dev_manifests_deploy_dir = dev_manifests_dir.join(DEPLOYMENT_DIR);
        let dev_manifests_abis_depl_dir = dev_manifests_deploy_dir.join(ABIS_DIR);

        let dev_manifest_toml = dev_manifests_deploy_dir.join("manifest").with_extension("toml");
        let dev_manifest_json = dev_manifests_deploy_dir.join("manifest").with_extension("json");

        assert!(fs::read_dir(target_dev_dir).is_err(), "Expected 'target/dev' to be empty");
        assert!(fs::read_dir(target_release_dir).is_err(), "Expected 'target/release' to be empty");

        assert!(
            fs::read_dir(dev_manifests_base_dir).is_err(),
            "Expected 'manifests/dev/base' to be empty"
        );
        assert!(
            fs::read_dir(dev_manifests_abis_base_dir).is_err(),
            "Expected 'manifests/dev/base/abis' to be empty"
        );
        assert!(
            fs::read_dir(&dev_manifests_abis_depl_dir).is_ok(),
            "Expected 'manifests/dev/deployment/abis' to be empty"
        );

        assert!(
            fs::read_dir(release_manifests_base_dir).is_err(),
            "Expected 'manifests/release/base' to be empty"
        );
        assert!(
            fs::read_dir(release_manifests_abis_base_dir).is_err(),
            "Expected 'manifests/release/base/abis' to be empty"
        );

        assert!(dev_manifest_toml.exists(), "Expected 'manifest.toml' to exist");
        assert!(dev_manifest_json.exists(), "Expected 'manifest.json' to exist");

        let clean_cmd = CleanArgs { full: true, all_profiles: true };
        clean_cmd.run(&config).unwrap();

        assert!(
            fs::read_dir(&dev_manifests_abis_depl_dir).is_err(),
            "Expected 'manifests/dev/deployment/abis' to be empty"
        );
        assert!(!dev_manifest_toml.exists(), "Expected 'manifest.toml' to not exist");
        assert!(!dev_manifest_json.exists(), "Expected 'manifest.json' to not exist");
    }
}
