//! Saya executable entry point.
use clap::Parser;
use console::Style;
use tokio::signal::ctrl_c;

mod args;

use args::SayaArgs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SayaArgs::parse();
    config.init_logging()?;

    print_intro();

    // Spin up saya main loop to gather data from Katana.

    // Wait until Ctrl + C is pressed, then shutdown
    ctrl_c().await?;
    // handle.stop()?;

    Ok(())
}

fn print_intro() {
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

    println!(
        r"
DATA AVAILBILITY
==================
    ",
    );

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
