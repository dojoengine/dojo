use std::process::exit;

use clap::Parser;
use scarb_ui::{OutputFormat, Ui};
use sozo::{args::SozoArgs, cli_main};

fn main() {
    let args = SozoArgs::parse();

    let ui = Ui::new(args.ui_verbosity(), OutputFormat::Text);

    if let Err(err) = cli_main(args) {
        ui.anyhow(&err);
        exit(1);
    }
}
