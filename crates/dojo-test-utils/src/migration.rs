use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR};
use dojo_world::manifest::BaseManifest;
use dojo_world::migration::strategy::{prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use katana_primitives::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::felt;

pub fn prepare_migration(
    manifest_dir: Utf8PathBuf,
    target_dir: Utf8PathBuf,
) -> Result<MigrationStrategy> {
    // In testing, profile name is always dev.
    let profile_name = "dev";

    let manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    let world = WorldDiff::compute(manifest, None);

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

    let manifest = BaseManifest::load_from_path(
        &manifest_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    let world = WorldDiff::compute(manifest, None);

    let seed = cairo_short_string_to_felt(seed).unwrap();
    prepare_for_migration(world_address, seed, &target_dir, world)
}
