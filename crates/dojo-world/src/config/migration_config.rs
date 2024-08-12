use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MigrationConfig {
    pub skip_contracts: Vec<String>,
}
