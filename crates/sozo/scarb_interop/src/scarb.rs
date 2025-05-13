use std::env;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use anyhow::{Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};

use crate::Config;
use crate::fsx::{self, PathBufUtf8Ext};

pub const MANIFEST_FILE_NAME: &str = "Scarb.toml";

pub struct Scarb {}

impl Scarb {
    fn execute(current_dir: &Utf8Path, args: Vec<&str>) -> Result<()> {
        let stdout = Command::new("scarb")
            .current_dir(current_dir)
            .args(args)
            .stdout(Stdio::piped())
            .spawn()?
            .stdout
            .ok_or_else(|| anyhow!("Could not capture standard output."))?;

        let reader = BufReader::new(stdout);
        reader.lines().filter_map(|line| line.ok()).for_each(|line| println!("{}", line));

        Ok(())
    }

    pub fn build(config: &Config) -> Result<()> {
        Self::execute(config.manifest_dir(), vec!["build"])
    }

    pub fn test(config: &Config) -> Result<()> {
        Self::execute(config.manifest_dir(), vec!["test"])
    }
}

pub fn find_manifest_path(user_override: Option<&Utf8Path>) -> Result<Utf8PathBuf> {
    match user_override {
        Some(user_override) => Ok(fsx::canonicalize(user_override)
            .unwrap_or_else(|_| user_override.into())
            .try_into_utf8()?),
        None => {
            let pwd = fsx::canonicalize_utf8(env::current_dir()?)?;
            let accept_all = |_| Ok(true);
            let manifest_path = try_find_manifest_of_pwd(pwd.clone(), accept_all)?
                .unwrap_or_else(|| pwd.join(MANIFEST_FILE_NAME));
            Ok(manifest_path)
        }
    }
}

fn try_find_manifest_of_pwd(
    pwd: Utf8PathBuf,
    accept: impl Fn(Utf8PathBuf) -> Result<bool>,
) -> Result<Option<Utf8PathBuf>> {
    let mut root = Some(pwd.as_path());
    while let Some(path) = root {
        let manifest = path.join(MANIFEST_FILE_NAME);
        if manifest.is_file() && accept(manifest.clone())? {
            return Ok(Some(manifest));
        }
        root = path.parent();
    }
    Ok(None)
}
