use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use dojo_world::manifest::{
    AbstractManifestError, BaseManifest, DeploymentManifest, OverlayManifest,
};
use scarb_ui::Ui;
use starknet::accounts::ConnectedAccount;
use starknet_crypto::FieldElement;

use super::ui::MigrationUi;

/// Loads:
///     - `BaseManifest` from filesystem
///     - `DeployedManifest` from onchain data if `world_address` is `Some`
pub(super) async fn load_world_manifests<A>(
    manifest_dir: &Utf8PathBuf,
    overlay_dir: &Utf8PathBuf,
    account: A,
    world_address: Option<FieldElement>,
    ui: &Ui,
    skip_migration: Option<Vec<String>>,
) -> Result<(BaseManifest, Option<DeploymentManifest>)>
where
    A: ConnectedAccount + Sync + Send,
    <A as ConnectedAccount>::Provider: Send,
{
    ui.print_step(1, "🌎", "Building World state...");

    let mut local_manifest = BaseManifest::load_from_path(manifest_dir)
        .map_err(|e| anyhow!("Fail to load local manifest file: {e}."))?;

    if let Some(skip_manifests) = skip_migration {
        local_manifest.remove_tags(skip_manifests);
    }

    if overlay_dir.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(overlay_dir, &local_manifest)
            .map_err(|e| anyhow!("Fail to load overlay manifest file: {e}."))?;

        // merge user defined changes to base manifest
        local_manifest.merge(overlay_manifest);
    }

    let remote_manifest = if let Some(address) = world_address {
        match DeploymentManifest::load_from_remote(account.provider(), address).await {
            Ok(manifest) => {
                ui.print_sub(format!("Found remote World: {address:#x}"));
                Some(manifest)
            }
            Err(AbstractManifestError::RemoteWorldNotFound) => None,
            Err(e) => {
                ui.verbose(format!("{e:?}"));
                return Err(anyhow!("Failed to build remote World state: {e}"));
            }
        }
    } else {
        None
    };

    if remote_manifest.is_none() {
        ui.print_sub("No remote World found");
    }

    Ok((local_manifest, remote_manifest))
}
