use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::Result;

use super::upload_service::UploadService;

/// Mock implementation of UploadService to be used for tests only.
/// It just stores uri and data in a HashMap when `upload` is called,
/// and returns these data when `get` is called.
#[derive(Debug, Default)]
pub struct MockUploadService {
    data: HashMap<String, Vec<u8>>,
}

#[allow(async_fn_in_trait)]
impl UploadService for MockUploadService {
    async fn upload(&mut self, data: Vec<u8>) -> Result<String> {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let hash = hasher.finish();

        let uri = format!("ipfs://{:x}", hash);
        self.data.insert(uri.clone(), data);

        Ok(uri)
    }

    #[cfg(test)]
    async fn get(&self, uri: String) -> Result<Vec<u8>> {
        if !uri.starts_with("ipfs://") {
            return Err(anyhow::anyhow!("Invalid URI format. Expected ipfs:// prefix"));
        }
        self.data
            .get(&uri)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No data found for URI: {}", uri))
    }
}
