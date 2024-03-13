use std::fs;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_lang::compiler::{ABIS_DIR, MANIFESTS_DIR};
use scarb::core::Config;

#[derive(Debug, Args)]
pub struct CleanArgs {
    #[arg(short, long)]
    #[arg(help = "Remove manifests and abis only.")]
    #[arg(long_help = "Remove manifests and abis only.")]
    pub manifests_abis: bool,

    #[arg(short, long)]
    #[arg(help = "Remove artifacts only.")]
    #[arg(long_help = "Remove artifacts only.")]
    pub artifacts: bool,
}

impl CleanArgs {
    pub fn clean_manifests_abis(&self, root_dir: &Utf8PathBuf) -> Result<()> {
        let manifest_dir = root_dir.join(MANIFESTS_DIR);
        let abis_dir = root_dir.join(ABIS_DIR);

        if manifest_dir.exists() {
            fs::remove_dir_all(manifest_dir)?;
        }

        if abis_dir.exists() {
            fs::remove_dir_all(abis_dir)?;
        }

        Ok(())
    }

    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let clean_manifests_abis = self.manifests_abis || !self.artifacts;
        let clean_artifacts = self.artifacts || !self.manifests_abis;

        if clean_manifests_abis {
            let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
            self.clean_manifests_abis(&manifest_dir)?;
        }

        if clean_artifacts {
            scarb::ops::clean(config)?;
        }

        Ok(())
    }
}
