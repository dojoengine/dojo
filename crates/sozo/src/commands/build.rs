use anyhow::Result;
use clap::Args;
use dojo_lang::scarb_internal::compile_workspace;
use scarb::core::{Config, TargetKind};
use scarb::ops::CompileOpts;

#[derive(Args, Debug)]
pub struct BuildArgs;

impl BuildArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        compile_workspace(
            config,
            CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
        )
    }
}
