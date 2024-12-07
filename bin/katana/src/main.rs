#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cli;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    cli::Cli::parse().run()?;
    Ok(())
}
