use serde::Deserialize;

use crate::uri::Uri;

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceConfig {
    pub tag: String,
    pub description: Option<String>,
    pub icon_uri: Option<Uri>,
}
