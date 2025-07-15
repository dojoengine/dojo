use std::path::PathBuf;
use std::{fs, io};

use camino::Utf8PathBuf;

pub const SPAWN_AND_MOVE_TEST_DB_DIR: &str = "/tmp/spawn-and-move-db";
pub const TYPES_TEST_DB_DIR: &str = "/tmp/types-test-db";

/// Copies the spawn and move test database to a temporary directory and returns the path to the
/// temporary directory. Must be used if the test is going to modify the database.
pub fn copy_spawn_and_move_db() -> Utf8PathBuf {
    copy_tmp_dir(&Utf8PathBuf::from(SPAWN_AND_MOVE_TEST_DB_DIR))
}

/// Copies the types test database to a temporary directory and returns the path to the temporary
/// directory. Must be used if the test is going to modify the database.
pub fn copy_types_test_db() -> Utf8PathBuf {
    copy_tmp_dir(&Utf8PathBuf::from(TYPES_TEST_DB_DIR))
}

/// Copies a directory into a temporary directory.
///
/// # Returns
///
/// A [`Utf8PathBuf`] object pointing to the copied directory.
fn copy_tmp_dir(source_dir: &Utf8PathBuf) -> Utf8PathBuf {
    let temp_project_dir = Utf8PathBuf::from(
        assert_fs::TempDir::new().unwrap().to_path_buf().to_string_lossy().to_string(),
    );

    fn copy_dir_recursively(src: &PathBuf, dst: &PathBuf) -> io::Result<()> {
        if src.is_dir() {
            fs::create_dir_all(dst)?;
            for entry in fs::read_dir(src)? {
                let entry = entry?;
                let path = entry.path();
                let dst_path = dst.join(path.file_name().unwrap());
                if path.is_dir() {
                    copy_dir_recursively(&path, &dst_path)?;
                } else {
                    fs::copy(&path, &dst_path)?;
                }
            }
        } else {
            fs::copy(src, dst)?;
        }
        Ok(())
    }

    copy_dir_recursively(&source_dir.to_path_buf().into(), &temp_project_dir.to_path_buf().into())
        .unwrap_or_else(|e| panic!("Failed to copy directory: {}", e));

    temp_project_dir
}
