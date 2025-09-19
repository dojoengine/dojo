use anyhow::Result;
use clap::{Args, Subcommand};
use scarb_metadata::Metadata;
use sozo_ui::SozoUi;

use crate::WalnutDebugger;

#[derive(Debug, Args)]
pub struct WalnutArgs {
    #[command(subcommand)]
    pub command: WalnutVerifyCommand,
}

#[derive(Debug, Subcommand)]
pub enum WalnutVerifyCommand {
    #[command(
        about = "Verify contracts in walnut.dev - essential for debugging source code in Walnut"
    )]
    Verify(WalnutVerifyOptions),
}

#[derive(Debug, Args)]
pub struct WalnutVerifyOptions {}

impl WalnutArgs {
    pub async fn run(self, scarb_metadata: &Metadata, ui: &SozoUi) -> Result<()> {
        match self.command {
            WalnutVerifyCommand::Verify(_options) => {
                WalnutDebugger::verify(scarb_metadata, ui).await?;
            }
        }
        Ok(())
    }
}
