use anyhow::Result;
use clap::Args;
use scarb::core::Config;
use sozo_walnut::WalnutDebugger;

#[derive(Debug, Args)]
pub struct VerifyArgs {}

impl VerifyArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        config.tokio_handle().block_on(async {
            WalnutDebugger::verify(&ws).await?;
            Ok(())
        })
    }
}
