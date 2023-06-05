use camino::Utf8PathBuf;
use sqlx::sqlite::SqlitePool;

use crate::state::sql::Sql;
use crate::state::State;

#[sqlx::test(migrations = "./migrations")]
async fn test_load_from_manifest(pool: SqlitePool) {
    let manifest = dojo_world::manifest::Manifest::load_from_path(
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev/manifest.json".into()).unwrap(),
    )
    .unwrap();

    let mut state = Sql::new(pool.clone()).unwrap();
    state.load_from_manifest(manifest).await.unwrap();
}
