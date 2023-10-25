use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct MetadataUpdateProcessor;

#[async_trait]
impl<P> EventProcessor<P> for MetadataUpdateProcessor
where
    P: Provider + Send + Sync,
{
    fn event_key(&self) -> String {
        "MetadataUpdate".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                "invalid keys for event {}: {}",
                <MetadataUpdateProcessor as EventProcessor<P>>::event_key(self),
                <MetadataUpdateProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block: &BlockWithTxs,
        _invoke_receipt: &InvokeTransactionReceipt,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let resource = &event.data[0];
        let uri_len: u8 = event.data[1].try_into().unwrap();

        let uri = if uri_len > 0 {
            event.data[2..=uri_len as usize + 1]
                .iter()
                .map(parse_cairo_short_string)
                .collect::<Result<Vec<_>, _>>()?
                .concat()
        } else {
            "".to_string()
        };

        info!("Resource {:#x} metadata set: {}", resource, uri);

        db.set_metadata(resource, uri);

        Ok(())
    }
}
