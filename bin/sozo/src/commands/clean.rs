use std::fs;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_lang::compiler::{ABIS_DIR, BASE_DIR, MANIFESTS_DIR};
use scarb::core::Config;

#[derive(Debug, Args)]
pub struct CleanArgs {
    #[arg(short, long)]
    #[arg(help = "Removes all the generated files, including scarb artifacts and ALL the \
                  manifests files.")]
    #[arg(long_help = "Removes all the generated files, including scarb artifacts and ALL the \
                       manifests files.")]
    pub all: bool,
}

impl CleanArgs {
    pub fn clean_manifests(&self, profile_dir: &Utf8PathBuf) -> Result<()> {
        let dirs = vec![profile_dir.join(BASE_DIR), profile_dir.join(ABIS_DIR).join(BASE_DIR)];

        for d in dirs {
            if d.exists() {
                fs::remove_dir_all(d)?;
            }
        }

        Ok(())
    }

    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let profile_name =
            ws.current_profile().expect("Scarb profile is expected at this point.").to_string();

        // Manifest path is always a file, we can unwrap safely to get the
        // parent folder.
        let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();

        let profile_dir = manifest_dir.join(MANIFESTS_DIR).join(profile_name);

        // By default, this command cleans the build manifests and scarb artifacts.
        scarb::ops::clean(config)?;
        self.clean_manifests(&profile_dir)?;

        if self.all && profile_dir.exists() {
            fs::remove_dir_all(profile_dir)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler;
    use dojo_test_utils::sequencer::{get_default_test_starknet_config, TestSequencer};
    use sozo_ops::migration;

    use super::*;

    #[test]
    fn test_clean() {
        let source_project = "../../examples/spawn-and-move/Scarb.toml";

        // Build a completely new project in it's own directory.
        let (temp_project_dir, config, _) = compiler::copy_build_project_temp(source_project, true);

        let sequencer = config.tokio_handle().block_on(async {
            TestSequencer::start(Default::default(), get_default_test_starknet_config()).await
        });

        let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

        // Plan the migration to generate some manifests other than base.
        config.tokio_handle().block_on(async {
            migration::migrate(
                &ws,
                None,
                "chain_id".to_string(),
                sequencer.url().to_string(),
                &sequencer.account(),
                Some("dojo_examples".to_string()),
                true,
                None,
            )
            .await
            .unwrap()
        });

        let clean_cmd = CleanArgs { all: false };
        clean_cmd.run(&config).unwrap();

        let profile_name = config.profile().to_string();

        let target_dev_dir = temp_project_dir.join("target").join(&profile_name);
        let profile_manifests_dir = temp_project_dir.join("manifests").join(&profile_name);
        let manifests_dev_base_dir = profile_manifests_dir.join("base");
        let manifests_dev_abis_base_dir = profile_manifests_dir.join("abis").join("base");
        let manifests_dev_abis_depl_dir = profile_manifests_dir.join("abis").join("deployments");
        let manifest_toml = profile_manifests_dir.join("manifest").with_extension("toml");
        let manifest_json = profile_manifests_dir.join("manifest").with_extension("json");

        assert!(fs::read_dir(&target_dev_dir).is_err(), "Expected 'target/dev' to be empty");
        assert!(
            fs::read_dir(&manifests_dev_base_dir).is_err(),
            "Expected 'manifests/dev/base' to be empty"
        );
        assert!(
            fs::read_dir(&manifests_dev_abis_base_dir).is_err(),
            "Expected 'manifests/dev/abis/base' to be empty"
        );
        assert!(
            fs::read_dir(&manifests_dev_abis_depl_dir).is_ok(),
            "Expected 'manifests/dev/abis/deployments' to not be empty"
        );
        assert!(manifest_toml.exists(), "Expected 'manifest.toml' to exist");
        assert!(manifest_json.exists(), "Expected 'manifest.json' to exist");

        let clean_cmd = CleanArgs { all: true };
        clean_cmd.run(&config).unwrap();

        assert!(
            fs::read_dir(&manifests_dev_abis_depl_dir).is_err(),
            "Expected 'manifests/dev/abis/deployments' to be empty"
        );
        assert!(!manifest_toml.exists(), "Expected 'manifest.toml' to not exist");
        assert!(!manifest_json.exists(), "Expected 'manifest.json' to not exist");

        sequencer.stop().unwrap();
    }
}
