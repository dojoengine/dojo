#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::process::exit;

use anyhow::{Result, bail};
use args::SozoArgs;
use camino::Utf8PathBuf;
use clap::Parser;
use commands::Commands;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use sozo_ui::{SozoUi, SozoUiTheme};
use tracing::trace;
mod args;
mod commands;
mod features;
mod utils;
use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};

#[tokio::main]
async fn main() {
    let args = SozoArgs::parse();

    let _ = args.init_logging(&args.verbose);

    let ui_theme = match theme_mode(QueryOptions::default()).unwrap() {
        ThemeMode::Light => SozoUiTheme::light(),
        ThemeMode::Dark => SozoUiTheme::dark(),
    };

    let ui = SozoUi::new(ui_theme, args.ui_verbosity());

    if let Err(err) = cli_main(args, &ui).await {
        ui.error(format!("{err:?}").trim());
        exit(1);
    }
}

async fn cli_main(args: SozoArgs, ui: &SozoUi) -> Result<()> {
    if let Commands::Init(args) = args.command {
        args.run(ui)
    } else {
        // Default to the current directory to mimic how Scarb works.
        let manifest_path = if let Some(manifest_path) = &args.manifest_path {
            manifest_path
        } else {
            let current_dir = Utf8PathBuf::from_path_buf(std::env::current_dir()?)
                .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in path: {}", e.display()))?;

            &current_dir.join("Scarb.toml")
        };

        if !manifest_path.exists() {
            bail!("Unable to find {}", &manifest_path);
        }

        let scarb_metadata =
            Metadata::load(manifest_path, args.profile_spec.determine()?.as_str(), args.offline)?;

        trace!(%scarb_metadata.runtime_manifest, "Configuration built successfully.");

        commands::run(args.command, &scarb_metadata, ui).await
    }
}
