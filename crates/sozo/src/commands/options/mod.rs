use scarb::core::Workspace;
use toml::Value;

pub mod account;
pub mod starknet;
pub mod world;

pub(super) fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Option<Value> {
    ws.current_package().ok()?.manifest.metadata.tool_metadata.as_ref()?.get("dojo").cloned()
}
