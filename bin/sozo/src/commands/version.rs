use std::process::Command;

use anyhow::{bail, Result};
use clap::Args;
use scarb_metadata::Metadata;

#[derive(Debug, Args)]
pub struct VersionArgs {}

// TODO RBA: this command should be removed as we already have the --version flag.
impl VersionArgs {
    pub fn run(&self, scarb_metadata: &Metadata) -> Result<()> {
        let Some(app) = &scarb_metadata.app_exe else {
            bail!(
                "Scarb not found. Find install instruction here: https://docs.swmansion.com/scarb"
            )
        };

        let output = Command::new(app).args(["--version"]).output()?;
        println!("{}", String::from_utf8(output.stdout)?);

        Ok(())
    }
}
