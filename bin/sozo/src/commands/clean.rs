use anyhow::Result;
use clap::Args;
use scarb_interop::MetadataDojoExt;
use scarb_metadata::Metadata;

#[derive(Debug, Args)]
pub struct CleanArgs {
    #[arg(long)]
    #[arg(help = "Clean all profiles.")]
    pub all_profiles: bool,
}

impl CleanArgs {
    pub fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        if self.all_profiles {
            scarb_metadata.clean_dir_all_profiles();
        } else {
            scarb_metadata.clean_dir_profile();
        }

        Ok(())
    }
}

// these tests assume `example/spawn-and-move` is build for `dev` and `release` profile,
// which are normally built by the `build.rs` of `dojo-test-utils`.
// #[cfg(test)]
// mod tests {
// use dojo_test_utils::compiler::CompilerTestSetup;
// use dojo_world::manifest::DEPLOYMENT_DIR;
// use dojo_world::metadata::ABIS_DIR;
// use scarb::compiler::Profile;
//
// use super::*;
//
// #[test]
// fn default_clean_works() {
// let setup = CompilerTestSetup::from_examples("../../crates/dojo/core", "../../examples/");
// let config = setup.build_test_config("spawn-and-move", Profile::DEV);
//
// let temp_project_dir = config.manifest_path().parent().unwrap().to_path_buf();
//
// println!("temp_project_dir: {:?}", temp_project_dir);
//
// let clean_cmd = CleanArgs { full: false, all_profiles: false };
// clean_cmd.run(&config).unwrap();
//
// let dev_profile_name = "dev";
// let release_profile_name = "release";
//
// let target_dev_dir = temp_project_dir.join("target").join(dev_profile_name);
// let target_release_dir = temp_project_dir.join("target").join(release_profile_name);
//
// let dev_manifests_dir = temp_project_dir.join("manifests").join(dev_profile_name);
// let release_manifests_dir = temp_project_dir.join("manifests").join(release_profile_name);
//
// let dev_manifests_base_dir = dev_manifests_dir.join("base");
// let dev_manifests_abis_base_dir = dev_manifests_dir.join("base").join("abis");
// let release_manifests_base_dir = release_manifests_dir.join("base");
// let release_manifests_abis_base_dir = release_manifests_dir.join("base").join("abis");
//
// let dev_manifests_depl_dir = dev_manifests_dir.join("deployment");
// let dev_manifests_abis_depl_dir = dev_manifests_depl_dir.join("abis");
// let dev_manifest_toml = dev_manifests_depl_dir.join("manifest").with_extension("toml");
// let dev_manifest_json = dev_manifests_depl_dir.join("manifest").with_extension("json");
//
// assert!(fs::read_dir(target_dev_dir).is_err(), "Expected 'target/dev' to be empty");
// assert!(
// fs::read_dir(target_release_dir).is_ok(),
// "Expected 'target/release' to be present"
// );
//
// assert!(
// fs::read_dir(dev_manifests_base_dir).is_err(),
// "Expected 'manifests/dev/base' to be empty"
// );
// assert!(
// fs::read_dir(dev_manifests_abis_base_dir).is_err(),
// "Expected 'manifests/dev/base/abis' to be empty"
// );
// assert!(
// fs::read_dir(&dev_manifests_abis_depl_dir).is_ok(),
// "Expected 'manifests/dev/deployment/abis' to be non empty"
// );
//
// we expect release profile to be not affected
// assert!(
// fs::read_dir(release_manifests_base_dir).is_ok(),
// "Expected 'manifests/release/base' to be non empty"
// );
// assert!(
// fs::read_dir(release_manifests_abis_base_dir).is_ok(),
// "Expected 'manifests/release/base/abis' to be non empty"
// );
//
// assert!(dev_manifest_toml.exists(), "Expected 'manifest.toml' to exist");
// assert!(dev_manifest_json.exists(), "Expected 'manifest.json' to exist");
//
// let clean_cmd = CleanArgs { full: true, all_profiles: false };
// clean_cmd.run(&config).unwrap();
//
// assert!(
// fs::read_dir(&dev_manifests_abis_depl_dir).is_err(),
// "Expected 'manifests/dev/deployment/abis' to be empty"
// );
// assert!(!dev_manifest_toml.exists(), "Expected 'manifest.toml' to not exist");
// assert!(!dev_manifest_json.exists(), "Expected 'manifest.json' to not exist");
// }
//
// #[test]
// fn all_profile_clean_works() {
// let setup = CompilerTestSetup::from_examples("../../crates/dojo/core", "../../examples/");
//
// let config = setup.build_test_config("spawn-and-move", Profile::DEV);
//
// let temp_project_dir = config.manifest_path().parent().unwrap().to_path_buf();
//
// let clean_cmd = CleanArgs { full: false, all_profiles: true };
// clean_cmd.run(&config).unwrap();
//
// let dev_profile_name = "dev";
// let release_profile_name = "release";
//
// let target_dev_dir = temp_project_dir.join("target").join(dev_profile_name);
// let target_release_dir = temp_project_dir.join("target").join(release_profile_name);
//
// let dev_manifests_dir = temp_project_dir.join(MANIFESTS_DIR).join(dev_profile_name);
// let release_manifests_dir = temp_project_dir.join(MANIFESTS_DIR).join(release_profile_name);
//
// let dev_manifests_base_dir = dev_manifests_dir.join(BASE_DIR);
// let dev_manifests_abis_base_dir = dev_manifests_base_dir.join(ABIS_DIR);
// let release_manifests_base_dir = release_manifests_dir.join(BASE_DIR);
// let release_manifests_abis_base_dir = release_manifests_base_dir.join(ABIS_DIR);
//
// let dev_manifests_deploy_dir = dev_manifests_dir.join(DEPLOYMENT_DIR);
// let dev_manifests_abis_depl_dir = dev_manifests_deploy_dir.join(ABIS_DIR);
//
// let dev_manifest_toml = dev_manifests_deploy_dir.join("manifest").with_extension("toml");
// let dev_manifest_json = dev_manifests_deploy_dir.join("manifest").with_extension("json");
//
// assert!(fs::read_dir(target_dev_dir).is_err(), "Expected 'target/dev' to be empty");
// assert!(fs::read_dir(target_release_dir).is_err(), "Expected 'target/release' to be empty");
//
// assert!(
// fs::read_dir(dev_manifests_base_dir).is_err(),
// "Expected 'manifests/dev/base' to be empty"
// );
// assert!(
// fs::read_dir(dev_manifests_abis_base_dir).is_err(),
// "Expected 'manifests/dev/base/abis' to be empty"
// );
// assert!(
// fs::read_dir(&dev_manifests_abis_depl_dir).is_ok(),
// "Expected 'manifests/dev/deployment/abis' to be empty"
// );
//
// assert!(
// fs::read_dir(release_manifests_base_dir).is_err(),
// "Expected 'manifests/release/base' to be empty"
// );
// assert!(
// fs::read_dir(release_manifests_abis_base_dir).is_err(),
// "Expected 'manifests/release/base/abis' to be empty"
// );
//
// assert!(dev_manifest_toml.exists(), "Expected 'manifest.toml' to exist");
// assert!(dev_manifest_json.exists(), "Expected 'manifest.json' to exist");
//
// let clean_cmd = CleanArgs { full: true, all_profiles: true };
// clean_cmd.run(&config).unwrap();
//
// assert!(
// fs::read_dir(&dev_manifests_abis_depl_dir).is_err(),
// "Expected 'manifests/dev/deployment/abis' to be empty"
// );
// assert!(!dev_manifest_toml.exists(), "Expected 'manifest.toml' to not exist");
// assert!(!dev_manifest_json.exists(), "Expected 'manifest.json' to not exist");
// }
// }
