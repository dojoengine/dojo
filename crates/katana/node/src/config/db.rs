use std::path::PathBuf;

/// Database configurations.
#[derive(Debug, Clone, Default)]
pub struct DbConfig {
    /// The path to the database directory.
    pub dir: Option<PathBuf>,
}
