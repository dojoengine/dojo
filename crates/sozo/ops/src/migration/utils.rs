use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use dojo_world::contracts::naming::get_namespace_from_tag;
use dojo_world::contracts::WorldContract;
use dojo_world::manifest::{
    AbstractManifestError, BaseManifest, DeploymentManifest, OverlayManifest,
};
use dojo_world::migration::world::WorldDiff;
use itertools::Itertools;
use scarb_ui::Ui;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;

use super::ui::MigrationUi;
use crate::auth::{get_resource_selector, ResourceType};

/// Loads:
///     - `BaseManifest` from filesystem
///     - `DeployedManifest` from onchain data if `world_address` is `Some`
pub(super) async fn load_world_manifests<A>(
    manifest_dir: &Utf8PathBuf,
    overlay_dir: &Utf8PathBuf,
    account: A,
    world_address: Option<Felt>,
    ui: &Ui,
    skip_migration: &Option<Vec<String>>,
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

pub async fn generate_resource_map<A>(
    ui: &Ui,
    world: &WorldContract<A>,
    diff: &WorldDiff,
) -> Result<HashMap<String, ResourceType>>
where
    A: ConnectedAccount + Sync + Send,
    <A as Account>::SignError: 'static,
{
    let mut resource_map = HashMap::new();

    for contract in diff.contracts.iter() {
        let resource = ResourceType::Contract(contract.tag.clone());
        // we know the tag already contains the namespace
        let default_namespace = get_namespace_from_tag(&contract.tag);
        let selector =
            get_resource_selector(ui, world, &resource, &default_namespace).await.with_context(
                || format!("Failed to get resource selector for contract: {}", contract.tag),
            )?;

        resource_map.insert(selector.to_hex_string(), resource);
    }

    for model in diff.models.iter() {
        let resource = ResourceType::Model(model.tag.clone());
        // we know the tag already contains the namespace
        let default_namespace = get_namespace_from_tag(&model.tag);
        let selector = get_resource_selector(ui, world, &resource, &default_namespace)
            .await
            .with_context(|| format!("Failed to get resource selector for model: {}", model.tag))?;

        resource_map.insert(selector.to_hex_string(), resource);
    }

    // Collect all the namespaces from the contracts and models
    let namespaces = {
        let mut namespaces =
            diff.models.iter().map(|m| get_namespace_from_tag(&m.tag)).collect::<Vec<_>>();

        namespaces.extend(
            diff.contracts.iter().map(|c| get_namespace_from_tag(&c.tag)).collect::<Vec<_>>(),
        );

        // remove duplicates
        namespaces.into_iter().unique().collect::<Vec<_>>()
    };

    for namespace in &namespaces {
        let resource = ResourceType::Namespace(namespace.clone());
        let selector =
            get_resource_selector(ui, world, &resource, "").await.with_context(|| {
                format!("Failed to get resource selector for namespace: {}", namespace)
            })?;

        resource_map.insert(selector.to_hex_string(), resource);
    }

    Ok(resource_map)
}
