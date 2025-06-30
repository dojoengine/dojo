use anyhow::Result;
use camino::Utf8PathBuf;
use sozo_mcp::SozoMcpServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get manifest path from environment variable if set
    let manifest_path = std::env::var("MANIFEST_PATH").ok().map(Utf8PathBuf::from);

    // Create and serve the MCP server
    let server = SozoMcpServer::new(manifest_path);
    server.serve_stdio().await?;

    Ok(())
}
