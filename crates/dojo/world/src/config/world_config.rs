use std::collections::HashMap;

use serde::Deserialize;
use url::Url;

use crate::uri::Uri;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorldConfig {
    pub name: String,
    pub seed: String,
    pub description: Option<String>,
    pub cover_uri: Option<Uri>,
    pub icon_uri: Option<Uri>,
    pub website: Option<Url>,
    pub socials: Option<HashMap<String, String>>,
}
