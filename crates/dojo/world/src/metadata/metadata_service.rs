use anyhow::Result;

/// MetadataService trait to be implemented to upload
/// some metadata on a specific storage system.
#[allow(async_fn_in_trait)]
pub trait MetadataService: std::marker::Send + std::marker::Sync + std::marker::Unpin {
    /// Upload some bytes (`data`) to the storage system,
    /// and get back a string URI.
    ///
    /// # Arguments
    ///   * `data` - bytes to upload
    ///
    /// # Returns
    ///   A string URI or a Anyhow error.
    async fn upload(&mut self, data: Vec<u8>) -> Result<String>;

    /// Read stored bytes from a URI. (for tests only)
    ///
    /// # Arguments
    ///   * `uri` - the URI of the data to read
    ///
    /// # Returns
    ///  the read bytes or a Anyhow error.
    #[cfg(test)]
    async fn get(&self, uri: String) -> Result<Vec<u8>>;
}
