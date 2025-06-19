//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command.
//! This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use std::env;

use anyhow::Result;
use axum::{
    Router,
    extract::{Json, State},
    response::Json as JsonResponse,
    routing::{get, post},
};
use camino::Utf8PathBuf;
use clap::Args;
use itertools::Itertools;
use scarb::core::Config;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::process::Command as AsyncCommand;
use tower_http::cors::{Any, CorsLayer};

use sozo_mcp::{AppState, SozoMcpServer};

#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    #[arg(long, default_value = "10300")]
    #[arg(help = "Port to start the MCP server on.")]
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
            server.start(self.port).await?;

            Ok(())
        })
    }
}
