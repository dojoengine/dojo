use anyhow::Result;
use clap::Args;
use scarb::core::Workspace;
use scarb::ops;

#[derive(Args, Debug)]
pub struct BuildArgs;

impl BuildArgs {
    pub fn run(self, ws: &Workspace<'_>) -> Result<()> {
        ops::compile(ws)
    }
}
