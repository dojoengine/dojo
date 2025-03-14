use anyhow::Result;
use clap::Args;
use katana_chain_spec::rollup::LocalChainConfigDir;
use katana_primitives::chain::ChainId;
use starknet::core::utils::parse_cairo_short_string;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    /// The chain id.
    #[arg(value_parser = ChainId::parse)]
    chain: Option<ChainId>,
}

impl ConfigArgs {
    pub fn execute(self) -> Result<()> {
        match self.chain {
            Some(chain) => {
                let path = LocalChainConfigDir::open(&chain)?.config_path();
                let config = std::fs::read_to_string(&path)?;
                println!("File: {}\n\n{config}", path.display());
            }

            None => {
                let chains = katana_chain_spec::rollup::list()?;
                for chain in chains {
                    // TODO:
                    // We can't just assume that the id is a valid (and readable) ascii string
                    // as we don' yet handle that elegently in the `ChainId` type itself. The ids
                    // returned by `list` will be of the `ChainId::Id` variant and thus
                    // will display in hex form. But for now, it's fine to assume that because we
                    // only limit valid ASCII string in the `katana init` flow.
                    let name = parse_cairo_short_string(&chain.id())?;
                    println!("{name}");
                }
            }
        }
        Ok(())
    }
}
