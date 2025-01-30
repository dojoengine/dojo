use anyhow::Result;
use clap::Args;
use katana_chain_spec::rollup::file::ChainConfigDir;
use katana_primitives::chain::ChainId;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    /// The chain id.
    #[arg(value_parser = ChainId::parse)]
    chain: ChainId,
}

impl ConfigArgs {
    pub fn execute(self) -> Result<()> {
        let cs = ChainConfigDir::open(&self.chain)?;
        let path = cs.config_path();
        let config = std::fs::read_to_string(&path)?;
        println!("File: {}\n\n{config}", path.display());
        Ok(())
    }
}
