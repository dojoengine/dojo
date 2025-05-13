use core::fmt;

use camino::{Utf8Path, Utf8PathBuf};

use anyhow::Result;

#[derive(Debug)]
pub struct Config {
    manifest_path: Utf8PathBuf,
}

impl Config {
    pub fn builder(manifest_path: impl Into<Utf8PathBuf>) -> ConfigBuilder {
        ConfigBuilder::new(manifest_path.into())
    }

    pub fn manifest_path(&self) -> &Utf8Path {
        &self.manifest_path
    }

    pub fn manifest_dir(&self) -> &Utf8Path {
        &self.manifest_path.parent().unwrap()
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "manifest_path: {}", self.manifest_path)
    }
}

pub struct ConfigBuilder {
    manifest_path: Utf8PathBuf,
}

impl ConfigBuilder {
    fn new(manifest_path: Utf8PathBuf) -> Self {
        Self {
            manifest_path,
            /*
                       global_config_dir_override: None,
                       global_cache_dir_override: None,
                       path_env_override: None,
                       target_dir_override: None,
                       ui_verbosity: Verbosity::Normal,
                       ui_output_format: OutputFormat::Text,
                       offline: false,
                       log_filter_directive: None,
                       compilers: None,
                       cairo_plugins: None,
                       custom_source_patches: None,
                       tokio_handle: None,
                       profile: None,
            */
        }
    }

    pub fn build(self) -> Result<Config> {
        Ok(Config { manifest_path: self.manifest_path })
    }
}
