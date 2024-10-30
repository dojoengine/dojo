use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MigrationConfig {
    /// Contracts to skip during migration.
    /// Expecting tags.
    pub skip_contracts: Option<Vec<String>>,
    /// Disable multicall.
    pub disable_multicall: Option<bool>,
    /// Determine the contract initialization order.
    /// Expecting tags.
    pub order_inits: Option<Vec<String>>,
}
