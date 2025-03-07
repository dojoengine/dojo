#[derive(Debug, Clone)]
pub struct StarknetApiConfig {
    /// The max chunk size that can be served from the `getEvents` method.
    ///
    /// If `None`, the maximum chunk size is bounded by [`u64::MAX`].
    pub max_event_page_size: Option<u64>,

    /// The max keys whose proofs can be requested for from the `getStorageProof` method.
    ///
    /// If `None`, the maximum keys size is bounded by [`u64::MAX`].
    pub max_proof_keys: Option<u64>,

    /// Enable the execution of transactions from outside with Cartridge paymaster.
    #[cfg(feature = "cartridge")]
    pub use_cartridge_paymaster: bool,

    /// The root URL for the Cartridge API.
    #[cfg(feature = "cartridge")]
    pub cartridge_api_url: String,
}
