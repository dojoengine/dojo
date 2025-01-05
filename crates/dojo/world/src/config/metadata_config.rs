//! Metadata configuration for the world.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use crate::config::{ResourceConfig, WorldConfig};
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

impl Hash for WorldMetadata {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.seed.hash(state);
        self.description.hash(state);
        self.cover_uri.hash(state);
        self.icon_uri.hash(state);
        self.website.hash(state);

        json!(self.socials).to_string().hash(state);

        // include icon and cover data into the hash to
        // detect data changes even if the filename is the same.
        if let Some(Uri::File(icon)) = &self.icon_uri {
            let icon_data = std::fs::read(icon).expect("read icon failed");
            icon_data.hash(state);
        };

        if let Some(Uri::File(cover)) = &self.cover_uri {
            let cover_data = std::fs::read(cover).expect("read cover failed");
            cover_data.hash(state);
        };
    }
}

/// resource metadata that describes world resources such as contracts,
/// models or events.
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ResourceMetadata {
    pub name: String,
    pub description: Option<String>,
    pub icon_uri: Option<Uri>,
}

impl From<ResourceConfig> for ResourceMetadata {
    fn from(config: ResourceConfig) -> Self {
        ResourceMetadata {
            name: dojo_types::naming::get_name_from_tag(&config.tag),
            description: config.description,
            icon_uri: config.icon_uri,
        }
    }
}

impl Hash for ResourceMetadata {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.description.hash(state);
        self.icon_uri.hash(state);

        // include icon and cover data into the hash to
        // detect data changes even if the filename is the same.
        if let Some(Uri::File(icon)) = &self.icon_uri {
            let icon_data = std::fs::read(icon).expect("read icon failed");
            icon_data.hash(state);
        };
    }
}
