use url::Url;

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

    #[cfg(feature = "cartridge")]
    pub paymaster: Option<PaymasterConfig>,
}

#[derive(Debug, Clone)]
pub struct PaymasterConfig {
    /// The root URL for the Cartridge API.
    pub cartridge_api_url: Url,
}
