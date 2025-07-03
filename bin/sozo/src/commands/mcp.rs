//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command.
//! This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use anyhow::Result;
use clap::Args;
use scarb_metadata::Metadata;
use sozo_mcp::SozoMcpServer;

#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    #[arg(long, default_value = "0")]
    #[arg(help = "Port to start the MCP server on (HTTP mode only).")]
    pub port: u16,
}

impl McpArgs {
    pub async fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        let manifest_path = scarb_metadata.runtime_manifest.clone();

        let server = SozoMcpServer::new(Some(manifest_path));

        if self.port == 0 {
            server.serve_stdio().await.map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(())
        } else {
            anyhow::bail!("HTTP mode is not supported yet")
        }
    }
}
