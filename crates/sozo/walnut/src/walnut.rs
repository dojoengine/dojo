use anyhow::Result;
use clap::{Args, Subcommand};
use scarb_metadata::Metadata;
use scarb_ui::Ui;

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
    pub fn run(self, scarb_metadata: &Metadata, ui: &Ui) -> Result<()> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            match self.command {
                WalnutVerifyCommand::Verify(_options) => {
                    WalnutDebugger::verify(&scarb_metadata, ui).await?;
                }
            }
            Ok(())
        })
    }
}
