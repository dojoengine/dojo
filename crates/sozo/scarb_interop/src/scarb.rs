use std::io::{BufRead, BufReader, ErrorKind};
use std::process::{Command, Stdio};

use anyhow::{Result, anyhow, bail};
use camino::Utf8Path;

pub struct Scarb {}

impl Scarb {
    /// Executes a Scarb command for the given manifest path.
    fn execute(manifest_path: &str, args: Vec<&str>) -> Result<()> {
        // To not change the current dir at this level, we rely
        // on Scarb `manifest-path` option.
        let mut args_with_manifest = vec!["--manifest-path", manifest_path];

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
        reader.lines().filter_map(|line| line.ok()).for_each(|line| println!("{}", line));

        Ok(())
    }

    /// Builds the workspace provided in the Scarb metadata.
    ///
    /// Every Scarb project, even with one single package, are considered a workspace, with the `root` being the parent directory of the `Scarb.toml` file.
    pub fn build(manifest_path: &Utf8Path) -> Result<()> {
        Self::execute(&manifest_path.to_string(), vec!["build"])
    }

    /// Tests the workspace provided in the Scarb metadata.
    pub fn test(manifest_path: &Utf8Path) -> Result<()> {
        Self::execute(&manifest_path.to_string(), vec!["test"])
    }
}
