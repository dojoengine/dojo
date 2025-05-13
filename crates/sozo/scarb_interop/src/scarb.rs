use std::io::{BufRead, BufReader, ErrorKind};
use std::process::{Command, Stdio};

use anyhow::{Result, anyhow, bail};
use camino::Utf8Path;

use crate::Config;

pub struct Scarb {}

impl Scarb {
    fn execute(current_dir: &Utf8Path, args: Vec<&str>) -> Result<()> {
        let stdout = match Command::new("scarb")
            .current_dir(current_dir)
            .args(args)
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

    pub fn build(config: &Config) -> Result<()> {
        Self::execute(config.manifest_dir(), vec!["build"])
    }

    pub fn test(config: &Config) -> Result<()> {
        Self::execute(config.manifest_dir(), vec!["test"])
    }
}
