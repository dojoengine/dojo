use anyhow::Result;
use clap::Args;
use scarb::core::{Config, TargetKind};
use scarb::ops::{self, CompileOpts};

#[derive(Args, Debug)]
pub struct BuildArgs;

impl BuildArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let packages = ws.members().map(|p| p.id).collect();
        ops::compile(
            packages,
            CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
            &ws,
        )
    }
}
