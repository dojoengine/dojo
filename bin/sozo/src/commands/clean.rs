use std::fs;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_lang::compiler::{ABIS_DIR, BASE_DIR, MANIFESTS_DIR};
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
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let profile_name =
            ws.current_profile().expect("Scarb profile is expected at this point.").to_string();

        let clean_manifests_abis = self.manifests_abis || !self.artifacts;
        let clean_artifacts = self.artifacts || !self.manifests_abis;

        if clean_manifests_abis {
            let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
            self.clean_manifests_abis(&manifest_dir, &profile_name)?;
        }

        if clean_artifacts {
            scarb::ops::clean(config)?;
        }

        Ok(())
    }

    pub fn clean_manifests_abis(&self, root_dir: &Utf8PathBuf, profile_name: &str) -> Result<()> {
        let dirs = vec![
            root_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
            root_dir.join(MANIFESTS_DIR).join(profile_name).join(ABIS_DIR).join(BASE_DIR),
        ];

        for d in dirs {
            if d.exists() {
                fs::remove_dir_all(d)?;
            }
        }

        Ok(())
    }
}
