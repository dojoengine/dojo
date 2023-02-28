#[cfg(test)]
mod test;

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use camino::Utf8Path;
use scarb::core::Config;
use scarb::metadata::{MetadataOptions, MetadataVersion, ProjectMetadata};
use scarb::ops;
use scarb::ui::Verbosity;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use starknet::core::types::FieldElement;

#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error, Debug)]
pub enum DeserializationError {
    #[error(transparent)]
    TomlError(#[from] toml::de::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("PathError")]
    PathError,
}
const PROJECT_FILE_NAME: &str = "world.toml";

/// Dojo project config, including its file content and metadata about the file.
/// This file is expected to be at a root of a crate and specify the crate name and location and
/// of its dependency crates.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ProjectConfig {
    pub base_path: PathBuf,
    pub content: ProjectConfigContent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldConfig {
    pub address: Option<FieldElement>,
    pub initializer_class_hash: Option<FieldElement>,
}

pub struct DeploymentConfig {
    pub rpc: Option<String>,
}

/// Contents of a Dojo project config file.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfigContent {
    pub crate_roots: HashMap<SmolStr, PathBuf>,
    pub world: WorldConfig,
    pub deployments: Option<Deployments>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployments {
    pub testnet: Option<Deployment>,
    pub mainnet: Option<Deployment>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub rpc: Option<String>,
}

impl ProjectConfig {
    pub fn from_directory(directory: &Path) -> Result<Self, DeserializationError> {
        Self::from_file(&directory.join(PROJECT_FILE_NAME))
    }
    pub fn from_file(filename: &Path) -> Result<Self, DeserializationError> {
        let base_path = filename
            .parent()
            .and_then(|p| p.to_str())
            .ok_or(DeserializationError::PathError)?
            .into();
        let content = toml::from_str(&std::fs::read_to_string(filename)?)?;
        Ok(ProjectConfig { base_path, content })
    }
}

impl From<ProjectConfig> for cairo_lang_project::ProjectConfig {
    fn from(val: ProjectConfig) -> Self {
        cairo_lang_project::ProjectConfig {
            content: cairo_lang_project::ProjectConfigContent {
                crate_roots: val.content.crate_roots,
            },
            base_path: val.base_path,
            corelib: None,
        }
    }
}

pub fn read_metadata(path: Option<&Utf8Path>) -> Result<ProjectMetadata> {
    let manifest_path = ops::find_manifest_path(path).unwrap();

    let config = Config::builder(manifest_path)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();

    let opts = MetadataOptions { version: MetadataVersion::V1, no_deps: false };

    ProjectMetadata::collect(&ws, &opts)
}
