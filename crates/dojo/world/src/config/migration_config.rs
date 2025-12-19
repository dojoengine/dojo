use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestAbiFormat {
    AllInOne,
    PerContract,
}

impl Default for ManifestAbiFormat {
    fn default() -> Self {
        Self::AllInOne
    }
}

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
    /// Controls how ABIs are represented in the generated manifest.
    pub manifest_abi_format: Option<ManifestAbiFormat>,
}
