use camino::Utf8PathBuf;

pub const SPAWN_AND_MOVE_TEST_DB_DIR: &str = "/tmp/spawn-and-move-db";
pub const TYPES_TEST_DB_DIR: &str = "/tmp/types-test-db";

/// Copies the spawn and move test database to a temporary directory and returns the path to the
/// temporary directory. Must be used if the test is going to modify the database.
pub fn copy_spawn_and_move_db() -> Utf8PathBuf {
    crate::compiler::copy_tmp_dir(&Utf8PathBuf::from(SPAWN_AND_MOVE_TEST_DB_DIR))
}

/// Copies the types test database to a temporary directory and returns the path to the temporary
/// directory. Must be used if the test is going to modify the database.
pub fn copy_types_test_db() -> Utf8PathBuf {
    crate::compiler::copy_tmp_dir(&Utf8PathBuf::from(TYPES_TEST_DB_DIR))
}
