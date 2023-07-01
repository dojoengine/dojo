use anyhow::Result;
use clap::Args;
use scarb::core::Config;

#[derive(Args, Debug)]
pub struct EventsArgs {
    #[clap(short, long)]
    #[clap(help = "idk yet")]
    chunk_size: usize,
}

impl EventsArgs {
    pub fn run(self, _config: &Config) -> Result<()> {
        Ok(())
    }
}
