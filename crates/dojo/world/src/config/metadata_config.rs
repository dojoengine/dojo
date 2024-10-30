//! Metadata configuration for the world.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::WorldConfig;
use crate::uri::Uri;

/// World metadata that describes the world.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct WorldMetadata {
    pub name: String,
    pub seed: String,
    pub description: Option<String>,
    pub cover_uri: Option<Uri>,
    pub icon_uri: Option<Uri>,
    pub website: Option<Url>,
    pub socials: Option<HashMap<String, String>>,
}

impl From<WorldConfig> for WorldMetadata {
    fn from(config: WorldConfig) -> Self {
        WorldMetadata {
            name: config.name,
            seed: config.seed,
            description: config.description,
            cover_uri: config.cover_uri,
            icon_uri: config.icon_uri,
            website: config.website,
            socials: config.socials,
        }
    }
}
