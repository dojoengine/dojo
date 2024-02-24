use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR};
use dojo_world::manifest::BaseManifest;
use dojo_world::migration::strategy::{prepare_for_migration, MigrationStrategy};
use dojo_world::migration::world::WorldDiff;
use starknet::macros::felt;

pub fn prepare_migration(
    manifest_dir: Utf8PathBuf,
    target_dir: Utf8PathBuf,
) -> Result<MigrationStrategy> {
    let manifest =
        BaseManifest::load_from_path(&manifest_dir.join(MANIFESTS_DIR).join(BASE_DIR)).unwrap();
    let world = WorldDiff::compute(manifest, None);
    prepare_for_migration(None, Some(felt!("0x12345")), &target_dir, world)
}
