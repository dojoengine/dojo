use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use dojo_lang::compiler::{BASE_DIR, OVERLAYS_DIR};
use dojo_world::manifest::{
    AbstractManifestError, BaseManifest, DeploymentManifest, OverlayManifest,
};
use scarb_ui::Ui;
use starknet::accounts::{ConnectedAccount, SingleOwnerAccount};
use starknet::providers::Provider;
use starknet::signers::Signer;
use starknet_crypto::FieldElement;

use super::ui::MigrationUi;

/// Loads:
///     - `BaseManifest` from filesystem
///     - `DeployedManifest` from onchain dataa if `world_address` is `Some`
pub(super) async fn load_world_manifests<P, S>(
    profile_dir: &Utf8PathBuf,
    account: &SingleOwnerAccount<P, S>,
    world_address: Option<FieldElement>,
    ui: &Ui,
) -> Result<(BaseManifest, Option<DeploymentManifest>)>
where
    P: Provider + Sync + Send,
    S: Signer + Sync + Send,
{
    ui.print_step(1, "🌎", "Building World state...");

    let mut local_manifest = BaseManifest::load_from_path(&profile_dir.join(BASE_DIR))
        .map_err(|e| anyhow!("Fail to load local manifest file: {e}."))?;

    let overlay_path = profile_dir.join(OVERLAYS_DIR);
    if overlay_path.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(&profile_dir.join(OVERLAYS_DIR))
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
