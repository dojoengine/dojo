use anyhow::Result;
use clap::{Args, Subcommand};
use scarb_metadata::Metadata;
use sozo_ui::SozoUi;

use super::session::SessionArgs;

#[derive(Debug, Args)]
pub struct ControllerArgs {
    #[command(subcommand)]
    command: ControllerCommand,
}

#[derive(Debug, Subcommand)]
pub enum ControllerCommand {
    #[command(about = "Manage Cartridge controller sessions")]
    Session(Box<SessionArgs>),
}

impl ControllerArgs {
    pub async fn run(self, scarb_metadata: &Metadata, ui: &SozoUi) -> Result<()> {
        match self.command {
            ControllerCommand::Session(args) => args.run(scarb_metadata, ui).await,
        }
    }
}
