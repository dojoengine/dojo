use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use torii_client::contract::world::WorldContractReader;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct MetadataUpdateProcessor;

#[async_trait]
impl<P: Provider + Sync + 'static> EventProcessor<P> for MetadataUpdateProcessor {
    fn event_key(&self) -> String {
        "MetadataUpdate".to_string()
    }

    async fn process(
        &self,
        _world: &WorldContractReader<'_, P>,
        db: &mut Sql,
        _provider: &P,
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
