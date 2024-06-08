use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR};
use dojo_world::manifest::{BaseManifest, OverlayManifest};
use dojo_world::migration::strategy::{prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use katana_primitives::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::felt;

pub fn prepare_migration(
    manifest_dir: Utf8PathBuf,
    target_dir: Utf8PathBuf,
    skip_migration: Option<Vec<String>>,
) -> Result<MigrationStrategy> {
    // In testing, profile name is always dev.
    let profile_name = "dev";

    let mut manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    if let Some(skip_manifests) = skip_migration {
        for contract_or_model in skip_manifests {
            if let Some(index) = manifest.contracts.iter().position(|c| c.name == contract_or_model)
            {
                manifest.contracts.remove(index);
            } else if let Some(index) =
                manifest.models.iter().position(|m| m.name == contract_or_model)
            {
                manifest.models.remove(index);
            };
        }
    }

    let overlay_manifest = OverlayManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(OVERLAYS_DIR),
    )
    .unwrap();

    manifest.merge(overlay_manifest);

    let mut world = WorldDiff::compute(manifest, None);
    world.update_order().unwrap();

    prepare_for_migration(None, felt!("0x12345"), &target_dir, world)
}

pub fn prepare_migration_with_world_and_seed(
    manifest_dir: Utf8PathBuf,
    target_dir: Utf8PathBuf,
    world_address: Option<FieldElement>,
    seed: &str,
) -> Result<MigrationStrategy> {
    // In testing, profile name is always dev.
    let profile_name = "dev";

    let mut manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    let overlay_manifest = OverlayManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(OVERLAYS_DIR),
    )
    .unwrap();

    manifest.merge(overlay_manifest);

    let mut world = WorldDiff::compute(manifest, None);
    world.update_order().unwrap();

    let seed = cairo_short_string_to_felt(seed).unwrap();
    prepare_for_migration(world_address, seed, &target_dir, world)
}
