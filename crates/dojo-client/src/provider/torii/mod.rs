use async_trait::async_trait;
use cynic::http::ReqwestExt;
use starknet_crypto::FieldElement;

use super::Provider;

#[cynic::schema("world")]
mod schema {}

#[derive(Debug, thiserror::Error)]
pub enum ToriiProviderError {}

pub struct ToriiProvider {
    url: String,
    client: reqwest::Client,
}

impl ToriiProvider {
    pub fn new(url: String) -> Self {
        Self { client: reqwest::Client::new(), url }
    }

    fn build_entity_query(&self) {
        use cynic::QueryBuilder;
        todo!("build entity query")
    }
}

#[async_trait]
impl Provider for ToriiProvider {
    type Error = ToriiProviderError;

    async fn component(&self, name: &str) -> Result<FieldElement, Self::Error> {
        unimplemented!()
    }

    async fn system(&self, name: &str) -> Result<FieldElement, Self::Error> {
        unimplemented!()
    }

    async fn entity(
        &self,
        component: &str,
        keys: Vec<FieldElement>,
    ) -> Result<Vec<FieldElement>, Self::Error> {
        self.client.post(&self.url);
        unimplemented!()
    }
}
