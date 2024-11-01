use std::array::TryFromSliceError;
use std::fs::{self};
use std::io::{Read, Write};
use std::mem;
use std::path::{Path, PathBuf};

/// Current version of the database.
pub const CURRENT_DB_VERSION: u32 = 4;

/// Name of the version file.
const DB_VERSION_FILE_NAME: &str = "db.version";

#[derive(Debug, thiserror::Error)]
pub enum DatabaseVersionError {
    #[error("Database version file not found.")]
    FileNotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Malformed database version file: {0}")]
    MalformedContent(#[from] TryFromSliceError),
    #[error("Database version mismatch. Expected version {expected}, found version {found}.")]
    MismatchVersion { expected: u32, found: u32 },
}

/// Insert a version file at the given `path` with the specified `version`. If the `path` is a
/// directory, the version file will be created inside it. Otherwise, the version file will be
/// created exactly at `path`.
///
/// Ideally the version file should be included in the database directory.
///
/// # Errors
///
/// Will fail if all the directories in `path` has not already been created.
pub(super) fn create_db_version_file(
    path: impl AsRef<Path>,
    version: u32,
) -> Result<(), DatabaseVersionError> {
    let path = path.as_ref();
    let path = if path.is_dir() { default_version_file_path(path) } else { path.to_path_buf() };

    let mut file = fs::File::create(path)?;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_readonly(true);

    file.set_permissions(permissions)?;
    file.write_all(&version.to_be_bytes()).map_err(DatabaseVersionError::Io)
}

/// Check the version of the database at the given `path`.
///
/// Returning `Ok` if the version matches with [`CURRENT_DB_VERSION`], otherwise `Err` is returned.
pub(super) fn check_db_version(path: impl AsRef<Path>) -> Result<(), DatabaseVersionError> {
    let version = get_db_version(path)?;
    if version != CURRENT_DB_VERSION {
        Err(DatabaseVersionError::MismatchVersion { expected: CURRENT_DB_VERSION, found: version })
    } else {
        Ok(())
    }
}

/// Get the version of the database at the given `path`.
pub(super) fn get_db_version(path: impl AsRef<Path>) -> Result<u32, DatabaseVersionError> {
    let path = path.as_ref();
    let path = if path.is_dir() { default_version_file_path(path) } else { path.to_path_buf() };

    let mut file = fs::File::open(path).map_err(|_| DatabaseVersionError::FileNotFound)?;
    let mut buf: Vec<u8> = Vec::new();
    file.read_to_end(&mut buf)?;

    let bytes = <[u8; mem::size_of::<u32>()]>::try_from(buf.as_slice())?;
    Ok(u32::from_be_bytes(bytes))
}

pub(super) fn default_version_file_path(path: &Path) -> PathBuf {
    path.join(DB_VERSION_FILE_NAME)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_current_version() {
        use super::CURRENT_DB_VERSION;
        assert_eq!(CURRENT_DB_VERSION, 4, "Invalid current database version")
    }
}
