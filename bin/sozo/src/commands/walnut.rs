use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;
use sozo_walnut::WalnutDebugger;

#[derive(Debug, Args)]
pub struct WalnutArgs {
    #[command(subcommand)]
    pub command: WalnutVerifyCommand,
}

#[derive(Debug, Subcommand)]
pub enum WalnutVerifyCommand {
    #[command(about = "Verify contracts in walnut.dev - essential for debugging source code in Walnut")]
    Verify(WalnutVerifyOptions),
}

#[derive(Debug, Args)]
pub struct WalnutVerifyOptions {}

impl WalnutArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        config.tokio_handle().block_on(async {
            match self.command {
                WalnutVerifyCommand::Verify(_options) => {
                    WalnutDebugger::verify(&ws).await?;
                }
            }
            Ok(())
        })
    }
}
