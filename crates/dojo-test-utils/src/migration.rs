use std::path::PathBuf;

use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_world::{
    manifest::Manifest,
    migration::{
        strategy::{prepare_for_migration, MigrationStrategy},
        world::WorldDiff,
    },
};

use starknet::macros::felt;

pub fn prepare_migration(path: PathBuf) -> Result<MigrationStrategy> {
    let target_dir = Utf8PathBuf::from_path_buf(path).unwrap();
    let manifest = Manifest::load_from_path(target_dir.join("manifest.json")).unwrap();
    let world = WorldDiff::compute(manifest, None);
    prepare_for_migration(None, Some(felt!("0x12345")), target_dir, world)
}
