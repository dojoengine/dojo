use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_world::manifest::{BaseManifest, OverlayManifest, BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR};
use dojo_world::migration::strategy::{prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use starknet::core::types::Felt;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::felt;

pub fn prepare_migration(
    manifest_dir: Utf8PathBuf,
    target_dir: Utf8PathBuf,
    skip_migration: Option<Vec<String>>,
    default_namespace: &str,
) -> Result<MigrationStrategy> {
    // In testing, profile name is always dev.
    let profile_name = "dev";

    let mut manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    if let Some(skip_manifests) = skip_migration {
        manifest.remove_tags(skip_manifests);
    }

    let overlay_dir = manifest_dir.join(OVERLAYS_DIR).join(profile_name);

    if overlay_dir.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(&overlay_dir, &manifest).unwrap();
        manifest.merge(overlay_manifest);
    }

    let world = WorldDiff::compute(manifest, None, default_namespace)?;

    let strat = prepare_for_migration(None, felt!("0x12345"), &target_dir, world).unwrap();

    Ok(strat)
}

pub fn prepare_migration_with_world_and_seed(
    manifest_dir: Utf8PathBuf,
    target_dir: Utf8PathBuf,
    world_address: Option<Felt>,
    seed: &str,
    default_namespace: &str,
) -> Result<(MigrationStrategy, WorldDiff, BaseManifest)> {
    // In testing, profile name is always dev.
    let profile_name = "dev";

    let mut manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    let overlay_dir = manifest_dir.join(OVERLAYS_DIR).join(profile_name);
    if overlay_dir.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(&overlay_dir, &manifest).unwrap();
        manifest.merge(overlay_manifest);
    }

    let world = WorldDiff::compute(manifest.clone(), None, default_namespace)?;

    let seed = cairo_short_string_to_felt(seed).unwrap();
    let strat = prepare_for_migration(world_address, seed, &target_dir, world.clone())?;
    Ok((strat, world, manifest))
}
