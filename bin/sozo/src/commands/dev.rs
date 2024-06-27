use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::Result;
use clap::Args;
use notify::event::Event;
use notify::{EventKind, PollWatcher, RecursiveMode, Watcher};
use scarb::core::Config;
use tracing::{error, info, trace};

use super::build::BuildArgs;
use super::migrate::MigrateArgs;
use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;

#[derive(Debug, Args)]
pub struct DevArgs {
    #[arg(long)]
    #[arg(help = "Name of the World.")]
    #[arg(long_help = "Name of the World. It's hash will be used as a salt when deploying the \
                       contract to avoid address conflicts.")]
    pub name: Option<String>,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,
}

impl DevArgs {
    /// Watches the `src` directory that is found at the same level of the `Scarb.toml` manifest
    /// of the project into the provided [`Config`].
    ///
    /// When a change is detected, it rebuilds the project and applies the migrations.
    pub fn run(self, config: &Config) -> Result<()> {
        let (tx, rx) = channel();

        let watcher_config = notify::Config::default().with_poll_interval(Duration::from_secs(1));

        let mut watcher = PollWatcher::new(tx, watcher_config)?;

        let watched_directory = config.manifest_path().parent().unwrap().join("src");

        watcher.watch(watched_directory.as_std_path(), RecursiveMode::Recursive).unwrap();

        // Always build the project before starting the dev loop to make sure that the project is
        // in a valid state. Devs may not use `build` anymore when using `dev`.
        BuildArgs::default().run(config)?;
        info!("Initial build completed.");

        let _ = MigrateArgs::new_apply(
            self.name.clone(),
            self.world.clone(),
            self.starknet.clone(),
            self.account.clone(),
        )
        .run(config);

        info!(
            directory = watched_directory.to_string(),
            "Initial migration completed. Waiting for changes."
        );

        let mut e_handler = EventHandler;

        loop {
            let is_rebuild_needed = match rx.recv() {
                Ok(maybe_event) => match maybe_event {
                    Ok(event) => e_handler.process_event(event),
                    Err(error) => {
                        error!(?error, "Processing event.");
                        break;
                    }
                },
                Err(error) => {
                    error!(?error, "Receiving event.");
                    break;
                }
            };

            if is_rebuild_needed {
                // Ignore the fails of those commands as the `run` function
                // already logs the error.
                let _ = BuildArgs::default().run(config);

                let _ = MigrateArgs::new_apply(
                    self.name.clone(),
                    self.world.clone(),
                    self.starknet.clone(),
                    self.account.clone(),
                )
                .run(config);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
struct EventHandler;

impl EventHandler {
    /// Processes a debounced event and return true if a rebuild is needed.
    /// Only considers Cairo file and the Scarb.toml manifest.
    fn process_event(&mut self, event: Event) -> bool {
        trace!(?event, "Processing event.");

        let paths = match event.kind {
            EventKind::Modify(_) => event.paths,
            EventKind::Remove(_) => event.paths,
            EventKind::Create(_) => event.paths,
            _ => vec![],
        };

        if paths.is_empty() {
            return false;
        }

        let mut is_rebuild_needed = false;

        for path in &paths {
            if let Some(filename) = path.file_name() {
                if filename == "Scarb.toml" {
                    info!("Rebuild to include Scarb.toml changes.");
                    is_rebuild_needed = true;
                } else if let Some(extension) = path.extension() {
                    if extension == "cairo" {
                        let file = path.to_string_lossy().to_string();
                        info!(file, "Rebuild from Cairo file change.");
                        is_rebuild_needed = true;
                    }
                }
            }
        }

        is_rebuild_needed
    }
}
