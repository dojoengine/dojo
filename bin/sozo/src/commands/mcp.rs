//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command.
//! This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use scarb::core::Config;

use crate::args::ProfileSpec;
use sozo_mcp::{SozoMcpServer, ServerMode};

#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    #[arg(long, default_value = "0")]
    #[arg(help = "Port to start the MCP server on (HTTP mode only).")]
    pub port: u16,
}

impl McpArgs {
    pub fn run(
        self,
        config: &Config,
        manifest_path: Option<Utf8PathBuf>,
    ) -> Result<()> {
        config.tokio_handle().block_on(async {
            let server = SozoMcpServer::new(manifest_path);
            
            let mode = if self.port == 0 {
                ServerMode::Stdio
            } else {
                ServerMode::Http { port: self.port }
            };
            
            server.start(mode).await?;

            Ok(())
        })
    }
}
