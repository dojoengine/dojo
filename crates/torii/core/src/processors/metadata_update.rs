use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Error, Result};
use async_trait::async_trait;
use base64::engine::general_purpose;
use base64::Engine as _;
use cainome::cairo_serde::{ByteArray, CairoSerde, Zeroable};
use dojo_world::contracts::world::WorldContractReader;
use dojo_world::metadata::WorldMetadata;
use dojo_world::uri::Uri;
use reqwest::Client;
use starknet::core::types::{Event, Felt};
use starknet::providers::Provider;
use tokio_util::bytes::Bytes;
use tracing::{error, info};

use super::EventProcessor;
use crate::sql::utils::I256;
use crate::sql::Sql;

const IPFS_URL: &str = "https://cartridge.infura-ipfs.io/ipfs/";
const MAX_RETRY: u8 = 3;

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

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                target: LOG_TARGET,
                event_key = %<MetadataUpdateProcessor as EventProcessor<P>>::event_key(self),
                invalid_keys = %<MetadataUpdateProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
                "Invalid event keys."
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _cache: Option<&mut HashMap<String, I256>>,
        _block_number: u64,
        block_timestamp: u64,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let resource = &event.data[0];
        let uri_str = ByteArray::cairo_deserialize(&event.data, 1)?.to_string()?;
        info!(
            target: LOG_TARGET,
            resource = %format!("{:#x}", resource),
            uri = %uri_str,
            "Resource metadata set."
        );
        db.set_metadata(resource, &uri_str, block_timestamp);

        let db = db.clone();
        let resource = *resource;

        // Only retrieve metadata for the World contract.
        if resource.is_zero() {
            tokio::spawn(async move {
                try_retrieve(db, resource, uri_str).await;
            });
        }

        Ok(())
    }
}

async fn try_retrieve(mut db: Sql, resource: Felt, uri_str: String) {
    match metadata(uri_str.clone()).await {
        Ok((metadata, icon_img, cover_img)) => {
            db.update_metadata(&resource, &uri_str, &metadata, &icon_img, &cover_img)
                .await
                .unwrap();
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

    let bytes = fetch_content(cid, MAX_RETRY).await?;
    let metadata: WorldMetadata = serde_json::from_str(std::str::from_utf8(&bytes)?)?;

    let icon_img = fetch_image(&metadata.icon_uri).await;
    let cover_img = fetch_image(&metadata.cover_uri).await;

    Ok((metadata, icon_img, cover_img))
}

async fn fetch_image(image_uri: &Option<Uri>) -> Option<String> {
    if let Some(uri) = image_uri {
        let data = fetch_content(uri.cid()?, MAX_RETRY).await.ok()?;
        let encoded = general_purpose::STANDARD.encode(data);
        return Some(encoded);
    }

    None
}

async fn fetch_content(cid: &str, mut retries: u8) -> Result<Bytes> {
    while retries > 0 {
        let response = Client::new().get(format!("{IPFS_URL}{}", cid)).send().await;

        match response {
            Ok(response) => return response.bytes().await.map_err(|e| e.into()),
            Err(e) => {
                retries -= 1;
                if retries > 0 {
                    info!(
                        target: LOG_TARGET,
                        error = %e,
                        "Fetch uri."
                    );
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    Err(Error::msg(format!(
        "Failed to pull data from IPFS after {} attempts, cid: {}",
        MAX_RETRY, cid
    )))
}
