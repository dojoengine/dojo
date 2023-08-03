use scarb::core::{ManifestMetadata, Workspace};
use toml::Value;

pub mod account;
pub mod starknet;
pub mod world;

pub(crate) fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Option<DojoMetadata> {
    Some(ws.current_package().ok()?.manifest.metadata.dojo_metadata())
}

pub(crate) struct DojoMetadata {
    env: Option<Value>,
}

impl DojoMetadata {
    pub fn env(&self) -> Option<Value> {
        self.env.clone()
    }
}
trait MetadataExt {
    fn dojo_metadata(&self) -> DojoMetadata;
}

impl MetadataExt for ManifestMetadata {
    fn dojo_metadata(&self) -> DojoMetadata {
        let dojo_metadata = self.tool_metadata.as_ref().and_then(|e| e.get("dojo")).cloned();
        let env_metadata = dojo_metadata.and_then(|inner| inner.get("env").cloned());
        DojoMetadata { env: env_metadata }
    }
}
