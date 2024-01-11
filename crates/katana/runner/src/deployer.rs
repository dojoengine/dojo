use crate::KatanaRunner;
use anyhow::{Context, Ok, Result};
use tokio::process::Command;

impl KatanaRunner {
    /// Known issue - rpc set in Scarb.toml overrides command line argument
    pub async fn deploy(&self, manifest: &str, script: &str) -> Result<()> {
        let rpc_url = &format!("http://localhost:{}", self.port);

        let out = Command::new("sozo")
            .arg("migrate")
            .args(["--rpc-url", rpc_url])
            .args(["--manifest-path", manifest])
            .output()
            .await
            .context("failed to start subprocess")?;

        if !out.status.success() {
            return Err(anyhow::anyhow!("deploy failed {:?}", out));
        }

        let out = Command::new("bash")
            .arg(script)
            .env("RPC_URL", rpc_url)
            .output()
            .await
            .context("failed to start script subprocess")?;

        if !out.status.success() {
            return Err(anyhow::anyhow!("script failed {:?}", out));
        }

        Ok(())
    }
}
