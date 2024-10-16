use anyhow::{Error, Result};
use async_trait::async_trait;
use base64::engine::general_purpose;
use base64::Engine as _;
use cainome::cairo_serde::Zeroable;
use dojo_world::config::WorldMetadata;
use dojo_world::contracts::abigen::world::Event as WorldEvent;
use dojo_world::contracts::world::WorldContractReader;
use dojo_world::uri::Uri;
use starknet::core::types::{Event, Felt};
use starknet::providers::Provider;
use tracing::{error, info};

use super::{EventProcessor, EventProcessorConfig};
use crate::sql::Sql;
use crate::utils::{fetch_content_from_ipfs, MAX_RETRY};

pub(crate) const LOG_TARGET: &str = "torii_core::processors::metadata_update";

#[derive(Default, Debug)]
pub struct MetadataUpdateProcessor;

#[async_trait]
impl<P> EventProcessor<P> for MetadataUpdateProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "MetadataUpdate".to_string()
    }

    fn validate(&self, _event: &Event) -> bool {
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        block_timestamp: u64,
        _event_id: &str,
        event: &Event,
        _config: &EventProcessorConfig,
    ) -> Result<(), Error> {
        // Torii version is coupled to the world version, so we can expect the event to be well
        // formed.
        let event = match WorldEvent::try_from(event).unwrap_or_else(|_| {
            panic!(
                "Expected {} event to be well formed.",
                <MetadataUpdateProcessor as EventProcessor<P>>::event_key(self)
            )
        }) {
            WorldEvent::MetadataUpdate(e) => e,
            _ => {
                unreachable!()
            }
        };

        // We know it's a valid Byte Array since it's coming from the world.
        let uri_str = event.uri.to_string().unwrap();
        info!(
            target: LOG_TARGET,
            resource = %format!("{:#x}", event.resource),
            uri = %uri_str,
            "Resource metadata set."
        );
        db.set_metadata(&event.resource, &uri_str, block_timestamp)?;

        let db = db.clone();

        // Only retrieve metadata for the World contract.
        if event.resource.is_zero() {
            tokio::spawn(async move {
                try_retrieve(db, event.resource, uri_str).await;
            });
        }

        Ok(())
    }
}

async fn try_retrieve(mut db: Sql, resource: Felt, uri_str: String) {
    match metadata(uri_str.clone()).await {
        Ok((metadata, icon_img, cover_img)) => {
            db.update_metadata(&resource, &uri_str, &metadata, &icon_img, &cover_img).unwrap();
            info!(
                target: LOG_TARGET,
                resource = %format!("{:#x}", resource),
                "Updated resource metadata from ipfs."
            );
        }
        Err(e) => {
            error!(
                target: LOG_TARGET,
                resource = %format!("{:#x}", resource),
                uri = %uri_str,
                error = %e,
                "Retrieving resource uri."
            );
        }
    }
}

async fn metadata(uri_str: String) -> Result<(WorldMetadata, Option<String>, Option<String>)> {
    let uri = Uri::Ipfs(uri_str);
    let cid = uri.cid().ok_or("Uri is malformed").map_err(Error::msg)?;

    let bytes = fetch_content_from_ipfs(cid, MAX_RETRY).await?;
    let metadata: WorldMetadata = serde_json::from_str(std::str::from_utf8(&bytes)?)?;

    let icon_img = fetch_image(&metadata.icon_uri).await;
    let cover_img = fetch_image(&metadata.cover_uri).await;

    Ok((metadata, icon_img, cover_img))
}

async fn fetch_image(image_uri: &Option<Uri>) -> Option<String> {
    if let Some(uri) = image_uri {
        let data = fetch_content_from_ipfs(uri.cid()?, MAX_RETRY).await.ok()?;
        let encoded = general_purpose::STANDARD.encode(data);
        return Some(encoded);
    }

    None
}
