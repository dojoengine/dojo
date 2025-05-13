#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::env;
use std::process::exit;

use anyhow::Result;
use args::SozoArgs;
use clap::Parser;

use scarb_ui::{OutputFormat, Ui};

use scarb_interop::{self, Config};
use tracing::trace;
mod args;
mod commands;
//mod utils;

fn main() {
    let args = SozoArgs::parse();
    let _ = args.init_logging(&args.verbose);
    let ui = Ui::new(args.ui_verbosity(), OutputFormat::Text);

    if let Err(err) = cli_main(args) {
        ui.anyhow(&err);
        exit(1);
    }
}

fn cli_main(args: SozoArgs) -> Result<()> {
    let manifest_path = scarb_interop::find_manifest_path(args.manifest_path.as_deref())?;

    // TODO RBA: utils::verify_cairo_version_compatibility(&manifest_path)?;

    let config = Config::builder(manifest_path.clone()).build()?;

    /* TODO RBA
       let config = Config::builder(manifest_path.clone())
           .log_filter_directive(env::var_os("SCARB_LOG"))
           .profile(args.profile_spec.determine()?)
           .offline(args.offline)
           .cairo_plugins(cairo_plugins)
           .ui_verbosity(args.ui_verbosity())
           .compilers(compilers)
           .build()?;
    */
    trace!(%manifest_path, "Configuration built successfully.");

    commands::run(args.command, &config)
}
