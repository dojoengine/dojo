use scarb::core::{ManifestMetadata, Workspace};
use serde::Deserialize;

pub fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Option<DojoMetadata> {
    Some(ws.current_package().ok()?.manifest.metadata.dojo())
}

#[derive(Default, Deserialize, Debug, Clone)]
pub struct DojoMetadata {
    pub env: Option<Environment>,
}

#[derive(Default, Deserialize, Clone, Debug)]
pub struct Environment {
    pub rpc_url: Option<String>,
    pub account_address: Option<String>,
    pub private_key: Option<String>,
    pub keystore_path: Option<String>,
    pub keystore_password: Option<String>,
    pub world_address: Option<String>,
}

impl Environment {
    pub fn world_address(&self) -> Option<&str> {
        self.world_address.as_deref()
    }

    pub fn rpc_url(&self) -> Option<&str> {
        self.rpc_url.as_deref()
    }

    pub fn account_address(&self) -> Option<&str> {
        self.account_address.as_deref()
    }

    pub fn private_key(&self) -> Option<&str> {
        self.private_key.as_deref()
    }

    #[allow(dead_code)]
    pub fn keystore_path(&self) -> Option<&str> {
        self.keystore_path.as_deref()
    }

    pub fn keystore_password(&self) -> Option<&str> {
        self.keystore_password.as_deref()
    }
}

impl DojoMetadata {
    pub fn env(&self) -> Option<&Environment> {
        self.env.as_ref()
    }
}
trait MetadataExt {
    fn dojo(&self) -> DojoMetadata;
}

impl MetadataExt for ManifestMetadata {
    fn dojo(&self) -> DojoMetadata {
        self.tool_metadata
            .as_ref()
            .and_then(|e| e.get("dojo"))
            .cloned()
            .map(|v| v.try_into::<DojoMetadata>().unwrap_or_default())
            .unwrap_or_default()
    }
}
