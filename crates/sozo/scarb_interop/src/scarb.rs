use std::io::{BufRead, BufReader, ErrorKind};
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Result};
use camino::Utf8Path;

#[derive(Debug)]
pub enum Features {
    NoDefault,
    AllFeatures,
    Features(String),
}

#[derive(Debug)]
pub struct Scarb {}

impl Scarb {
    /// Executes a Scarb command for the given manifest path.
    fn execute(manifest_path: &Utf8Path, args: Vec<&str>) -> Result<()> {
        // To not change the current dir at this level, we rely
        // on Scarb `manifest-path` option.
        let mut args_with_manifest = vec!["--manifest-path", manifest_path.as_str()];

        args_with_manifest.extend(args);

        let stdout = match Command::new("scarb")
            .args(&args_with_manifest)
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(child) => {
                child.stdout.ok_or_else(|| anyhow!("Could not capture standard output."))?
            }
            Err(err) => {
                let err = match err.kind() {
                    ErrorKind::NotFound =>
                    // TODO: have a better way to handle missing Scarb.
                    {
                        anyhow!(
                            "Scarb not found. Find install instruction here: https://docs.swmansion.com/scarb"
                        )
                    }
                    _ => anyhow!(err),
                };
                bail!(err);
            }
        };

        let reader = BufReader::new(stdout);
        reader.lines().map_while(|line| line.ok()).for_each(|line| println!("{}", line));

        Ok(())
    }

    fn format_packages(packages: &str) -> Vec<&str> {
        if packages.is_empty() { vec![] } else { vec!["--package", packages] }
    }

    fn format_features(features: &Features) -> Vec<&str> {
        match &features {
            Features::NoDefault => {
                vec!["--no-default-features"]
            }
            Features::AllFeatures => {
                vec!["--all-features"]
            }
            Features::Features(features) => {
                if features.is_empty() {
                    vec![]
                } else {
                    vec!["--features", features]
                }
            }
        }
    }

    pub fn build_simple_dev(manifest_path: &Utf8Path) -> Result<()> {
        Self::build(manifest_path, "dev", "", Features::Features("".to_string()), vec![])
    }

    /// Builds the workspace provided in the Scarb metadata.
    ///
    /// Every Scarb project, even with one single package, are considered a workspace,
    /// with the `root` being the parent directory of the `Scarb.toml` file.
    ///
    /// TODO: check if we should pass here directly the whole scarb metadata + the optional things
    /// from the CLI like features and packages. Or if having the manifest_path and the profile
    /// separated is a better approach.
    pub fn build(
        manifest_path: &Utf8Path,
        profile: &str,
        packages: &str,
        features: Features,
        other_args: Vec<&str>,
    ) -> Result<()> {
        let mut all_args = vec!["-P", profile, "build"];

        all_args.extend(Self::format_packages(packages));
        all_args.extend(Self::format_features(&features));
        all_args.extend(other_args);

        Self::execute(manifest_path, all_args)
    }

    /// Tests the workspace provided in the Scarb metadata.
    pub fn test(
        manifest_path: &Utf8Path,
        packages: &str,
        features: Features,
        other_args: Vec<&str>,
    ) -> Result<()> {
        let mut all_args = vec!["test"];

        all_args.extend(Self::format_packages(packages));
        all_args.extend(Self::format_features(&features));
        all_args.extend(other_args);

        Self::execute(manifest_path, all_args)
    }
}
