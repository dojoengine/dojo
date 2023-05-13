use bevy::app::App;
use bevy::ecs::system::Query;
use bevy::log;
use bevy::log::LogPlugin;
use bevy_dojo::indexer::apibara::core::node::v1alpha2::DataFinality;
use bevy_dojo::indexer::apibara::core::starknet::v1alpha2::{
    Block, FieldElement, Filter, HeaderFilter,
};
use bevy_dojo::indexer::apibara::sdk::{Configuration, DataMessage};
use bevy_dojo::prelude::*;
use chrono::{DateTime, Utc};

fn main() {
    let config = Configuration::<Filter>::default()
        .with_finality(DataFinality::DataStatusAccepted)
        .with_batch_size(10)
        .with_filter(build_filter);

    App::new()
        .set_runner(runner)
        .add_plugin(LogPlugin::default())
        .add_plugin(IndexerPlugin::<Filter, Block>::new_with_config(
            "https://mainnet.starknet.a5a.ch",
            config,
        ))
        .add_system(log_message)
        .run();
}

fn build_filter(f: Filter) -> Filter {
    let eth_address = FieldElement::from_hex(
        "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
    )
    .unwrap();
    let transfer_key =
        FieldElement::from_hex("0x99cd8bde557814842a3121e8ddfd433a539b8c9f14bf31ebf108d12e6196e9")
            .unwrap();

    f.with_header(HeaderFilter::weak()).add_event(|ev| {
        ev.with_from_address(eth_address.clone()).with_keys(vec![transfer_key.clone()])
    })
}

fn runner(mut app: App) {
    loop {
        app.update();
    }
}

fn log_message(query: Query<'_, '_, &IndexerMessage<Block>>) {
    query.iter().for_each(|msg| {
        match &msg.0 {
            DataMessage::Data { cursor, end_cursor, finality, batch } => {
                let start_block = cursor.clone().map(|c| c.order_key).unwrap_or_default();
                let end_block = end_cursor.order_key;

                println!(
                    "Received data from block {start_block} to {end_block} with finality \
                     {finality:?}"
                );

                for block in batch {
                    let header = block.clone().header.unwrap_or_default();

                    match TryInto::<DateTime<Utc>>::try_into(header.timestamp.unwrap_or_default()) {
                        Ok(timestamp) => {
                            println!("  Block {:>6} ({})", header.block_number, timestamp);

                            for event_with_tx in &block.events {
                                let event = event_with_tx.clone().event.unwrap_or_default();
                                let tx = event_with_tx.clone().transaction.unwrap_or_default();
                                let tx_hash =
                                    tx.meta.unwrap_or_default().hash.unwrap_or_default().to_hex();

                                let from_addr = event.data[0].to_hex();
                                let to_addr = event.data[1].to_hex();

                                println!(
                                    "    {} => {} ({})",
                                    &from_addr[..8],
                                    &to_addr[..8],
                                    &tx_hash[..8]
                                );
                            }
                        }
                        Err(e) => {
                            log::error!("{e}");
                        }
                    }
                }
            }
            DataMessage::Invalidate { cursor } => {
                log::error!("Chain reorganization detected: {cursor:?}");
            }
        };
    });
}
