use anyhow::{Context, Ok, Result};
use starknet::core::types::FieldElement;
use tokio::process::Command;

use crate::KatanaRunner;

impl KatanaRunner {
    /// Known issue - rpc set in Scarb.toml overrides command line argument
    pub async fn deploy(&self, manifest: &str, script: &str) -> Result<FieldElement> {
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

        let constract_address = String::from_utf8_lossy(&out.stdout)
            .lines()
            .rev()
            .find(|l| l.contains("> Contract address:"))
            .expect("failed to find contract address")
            .split_whitespace()
            .last()
            .expect("failed to get contract address")
            .to_owned();

        let constract_address = FieldElement::from_hex_be(&constract_address)
            .expect("failed to parse contract address");

        let out = Command::new("bash")
            .arg(script)
            .env("RPC_URL", rpc_url)
            .output()
            .await
            .context("failed to start script subprocess")?;

        if !out.status.success() {
            return Err(anyhow::anyhow!("script failed {:?}", out));
        }

        Ok(constract_address)
    }
}
