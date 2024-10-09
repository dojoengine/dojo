use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct DbConfig {
    pub dir: Option<PathBuf>,
}
