#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::process::exit;

use anyhow::Result;
use args::SozoArgs;
use camino::Utf8PathBuf;
use clap::Parser;

use scarb_ui::{OutputFormat, Ui};

use scarb_interop::MetadataErrorExt;
use scarb_metadata::MetadataCommand;
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
    // Default to the current directory to mimic how Scarb works.
    let manifest_path = if let Some(manifest_path) = &args.manifest_path {
        manifest_path
    } else {
        let current_dir = Utf8PathBuf::from_path_buf(std::env::current_dir()?)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in path: {}", e.display()))?;

        &current_dir.join("Scarb.toml")
    };

    let mut metadata = MetadataCommand::new();
    metadata.manifest_path(manifest_path);
    metadata.profile(args.profile_spec.determine()?.as_str());

    if args.offline {
        metadata.no_deps();
    }

    let scarb_metadata = match metadata.exec() {
        Ok(metadata) => metadata,
        Err(err) => {
            return Err(anyhow::anyhow!(err.format_error_message(&manifest_path)));
        }
    };

    // TODO RBA: utils::verify_cairo_version_compatibility(&manifest_path)?;

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
    trace!(%scarb_metadata.runtime_manifest, "Configuration built successfully.");

    commands::run(args.command, &scarb_metadata)
}
