use anyhow::Result;
use clap::Args;
use scarb::core::Config;
use scarb::ops;

#[derive(Args, Debug)]
pub struct BuildArgs;

impl BuildArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        ops::compile(&ws)
    }
}
