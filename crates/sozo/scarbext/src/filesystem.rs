use anyhow::Result;
use camino::Utf8Path;
use scarb::flock::Filesystem;

/// Handy enum for selecting the current profile or all profiles.
#[derive(Debug)]
pub enum ProfileSpec {
    WorkspaceCurrent,
    All,
}

/// Extension trait for the [`Filesystem`] type.
pub trait FilesystemExt {
    /// Returns a new Filesystem with the given subdirectories.
    ///
    /// This is a helper function since flock [`Filesystem`] only has a child method.
    fn children(&self, sub_dirs: &[impl AsRef<Utf8Path>]) -> Filesystem;

    /// Lists all the files in the filesystem root, not recursively.
    fn list_files(&self) -> Result<Vec<String>>;
}

impl FilesystemExt for Filesystem {
    fn children(&self, sub_dirs: &[impl AsRef<Utf8Path>]) -> Self {
        if sub_dirs.is_empty() {
            return self.clone();
        }

        let mut result = self.clone();

        for sub_dir in sub_dirs {
            result = result.child(sub_dir);
        }

        result
    }

    fn list_files(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();

        let path = self.to_string();

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }

        Ok(files)
    }
}
