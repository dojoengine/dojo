use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Args;
use notify::event::Event;
use notify::{EventKind, PollWatcher, RecursiveMode, Watcher};
use scarb::core::Config;
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use tracing::{error, info, trace};

use super::build::BuildArgs;
use super::migrate::MigrateArgs;
use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::commands::options::ipfs::IpfsOptions;
use crate::commands::options::verify::VerifyOptions;

#[derive(Debug, Args)]
pub struct DevArgs {
    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,

    #[command(flatten)]
    pub transaction: TransactionOptions,

    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript: bool,

    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript_v2: bool,

    #[arg(long)]
    #[arg(help = "Generate Unity bindings.")]
    pub unity: bool,

    #[arg(long)]
    #[arg(help = "Output directory.", default_value = "bindings")]
    pub bindings_output: String,

    /// Specify the features to activate.
    #[command(flatten)]
    pub features: FeaturesSpec,

    /// Specify packages to build.
    #[command(flatten)]
    pub packages: Option<PackagesFilter>,

    #[command(flatten)]
    pub verify: VerifyOptions,
}

impl DevArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let (file_tx, file_rx) = channel();
        let (rebuild_tx, rebuild_rx) = channel();

        let watcher_config =
            notify::Config::default().with_poll_interval(Duration::from_millis(500));

        let mut watcher = PollWatcher::new(file_tx, watcher_config)?;

        let watched_directory = config.manifest_path().parent().unwrap().join("src");
        watcher.watch(watched_directory.as_std_path(), RecursiveMode::Recursive).unwrap();

        // Initial build and migrate
        let build_args = BuildArgs {
            typescript: self.typescript,
            typescript_v2: self.typescript_v2,
            unity: self.unity,
            bindings_output: self.bindings_output,
            features: self.features,
            packages: self.packages,
            ..Default::default()
        };
        build_args.clone().run(config)?;
        info!("Initial build completed.");

        // As this `dev` command is for development purpose only,
        // allowing to watch for changes, compile and migrate them,
        // there is no need for metadata uploading. That's why,
        // `ipfs` is set to its default value meaning it is disabled.
        let migrate_args = MigrateArgs {
            world: self.world,
            starknet: self.starknet,
            account: self.account,
            transaction: self.transaction,
            verify: self.verify,
            ipfs: IpfsOptions::default(),
        };

        let _ = migrate_args.clone().run(config);

        info!(
            directory = watched_directory.to_string(),
            "Initial migration completed. Waiting for changes."
        );

        let e_handler = EventHandler;
        let rebuild_tx_clone = rebuild_tx.clone();

        // Independent thread to handle file events and trigger rebuilds.
        config.tokio_handle().spawn(async move {
            loop {
                match file_rx.recv() {
                    Ok(maybe_event) => match maybe_event {
                        Ok(event) => {
                            trace!(?event, "Event received.");

                            if e_handler.process_event(event) && rebuild_tx_clone.send(()).is_err()
                            {
                                break;
                            }
                        }
                        Err(error) => {
                            error!(?error, "Processing event.");
                            break;
                        }
                    },
                    Err(error) => {
                        error!(?error, "Receiving event.");
                        break;
                    }
                }
            }
        });

        // Main thread handles the rebuilds.
        let mut last_event_time = None;
        let debounce_period = Duration::from_millis(1500);

        loop {
            match rebuild_rx.try_recv() {
                Ok(()) => {
                    last_event_time = Some(Instant::now());
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if let Some(last_time) = last_event_time {
                        if last_time.elapsed() >= debounce_period {
                            let _ = build_args.clone().run(config);
                            let _ = migrate_args.clone().run(config);
                            last_event_time = None;
                        } else {
                            trace!("Change detected, waiting for debounce period.");
                        }
                    }
                    thread::sleep(Duration::from_millis(300));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct EventHandler;

impl EventHandler {
    /// Processes a debounced event and return true if a rebuild is needed.
    /// Only considers Cairo file and the Scarb.toml manifest.
    fn process_event(&self, event: Event) -> bool {
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
