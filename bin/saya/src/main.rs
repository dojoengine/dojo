//! Saya executable entry point.
use clap::Parser;
use console::Style;
use saya_core::{Saya, SayaConfig};
use tokio::signal::ctrl_c;

mod args;

use args::SayaArgs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = SayaArgs::parse();
    args.init_logging()?;

    let config = args.try_into()?;
    print_intro(&config);

    let saya = Saya::new(config).await?;
    saya.start().await?;

    // Wait until Ctrl + C is pressed, then shutdown
    ctrl_c().await?;
    // handle.stop()?;

    Ok(())
}

fn print_intro(config: &SayaConfig) {
    println!(
        "{}",
        Style::new().color256(94).apply_to(
            r"

 _______  _______           _______
(  ____ \(  ___  )|\     /|(  ___  )
| (    \/| (   ) |( \   / )| (   ) |
| (_____ | (___) | \ (_) / | (___) |
(_____  )|  ___  |  \   /  |  ___  |
      ) || (   ) |   ) (   | (   ) |
/\____) || )   ( |   | |   | )   ( |
\_______)|/     \|   \_/   |/     \|
"
        )
    );

    println!(
        r"
CONFIGURATION
=============
    ",
    );

    if let Some(da_config) = &config.data_availability {
        println!(
            r"
DATA AVAILBILITY
==================
{da_config}
    ",
        );
    }

    println!(
        r"
PROVER
==================
    ",
    );

    println!(
        r"
VERIFIER
==================
    ",
    );
}
