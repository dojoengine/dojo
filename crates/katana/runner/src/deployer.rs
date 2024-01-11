use crate::KatanaRunner;
use anyhow::{Context, Ok, Result};
use tokio::process::Command;

impl KatanaRunner {
    /// Known issue - rpc set in Scarb.toml overrides command line argument
    pub async fn deploy(&self, manifest: &str) -> Result<()> {
        let rpc_url = &format!("http://localhost:{}", self.port);

        let out = Command::new("sozo")
            .arg("migrate")
            .args(["--rpc-url", rpc_url])
            .args(["--manifest-path", manifest])
            .output()
            .await
            .context("failed to start subprocess")?;

        if out.status.success() {
            println!("deploy success");
        } else {
            return Err(anyhow::anyhow!("deploy failed {:?}", out));
        }

        Ok(())
    }
}
