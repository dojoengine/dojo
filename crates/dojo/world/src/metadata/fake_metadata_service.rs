use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use anyhow::Result;

use super::metadata_service::MetadataService;

/// Fake implementation of MetadataService to be used for tests only.
/// It just stores uri and data in a HashMap when `upload` is called,
/// and returns these data when `get` is called.

#[derive(Debug, Default)]
pub struct FakeMetadataService {
    data: HashMap<String, Vec<u8>>,
}

#[allow(async_fn_in_trait)]
impl MetadataService for FakeMetadataService {
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
        Ok(self.data.get(&uri).cloned().unwrap_or(Vec::<u8>::new()))
    }
}
