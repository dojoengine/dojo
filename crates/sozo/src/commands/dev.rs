use anyhow::Result;
use clap::Args;
use notify::event::{AccessMode, AccessKind, ModifyKind, RenameMode};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use scarb::core::Config;
use scarb::ops;

use std::sync::mpsc::channel;

#[derive(Args, Debug)]
pub struct DevArgs;

fn check_event(event: Event) -> bool {
    let matched = event.paths.iter().find(|&p| {
        if let Some(filename) = p.file_name() {
            if filename == "Scarb.toml" {
                return true;
            }
        }
        if let Some(extension) = p.extension() {
            if extension == "cairo" {
                return true;
            }
        }
        false
    });
    if matched.is_some() {
        match event.kind {
            EventKind::Access(AccessKind::Close(AccessMode::Write)) => return true,
            EventKind::Remove(_) => return true,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => return true,
            _ => return false,
        }
    }
    false
}

fn build(config: &Config) -> Result<()> {
    let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
    ops::compile(&ws)
}

impl DevArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let (tx, rx) = channel();
        // Automatically select the best implementation for your platform.
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;

        watcher.watch(config.manifest_path().parent().unwrap().as_std_path(), RecursiveMode::Recursive)?;
        let mut result = build(config);

        loop {
            let mut need_rebuild = false;
            match rx.recv() {
                Ok(event) => {
                    if event.is_ok() {
                        need_rebuild = check_event(event.ok().unwrap());
                    }
                }
                Err(error) => {
                    log::error!("Error: {error:?}");
                    break;
                }
            }
            if need_rebuild {
                result = build(config);
            }
        }
        result
    }
}
