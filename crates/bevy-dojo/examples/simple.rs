use apibara_core::node::v1alpha2::DataFinality;
use apibara_core::starknet::v1alpha2::{Block, FieldElement, Filter, HeaderFilter};
use apibara_sdk::{Configuration, DataMessage};
use bevy::app::App;
use bevy::ecs::system::Query;
use bevy_dojo::{IndexerMessage, IndexerPlugin};

fn main() {
    let config = Configuration::<Filter>::default()
        .with_finality(DataFinality::DataStatusAccepted)
        .with_batch_size(10)
        .with_filter(build_filter);

    App::new()
        .set_runner(runner)
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

fn log_message(query: Query<&IndexerMessage<Block>>) {
    query.iter().for_each(|msg| {
        match &msg.0 {
            DataMessage::Data { cursor, end_cursor, finality, .. } => {
                let start_block = cursor.as_ref().map(|c| c.order_key).unwrap_or_default();
                let end_block = end_cursor.order_key;

                println!(
                    "Received data from block {start_block} to {end_block} with finality \
                     {finality:?}"
                );
            }
            DataMessage::Invalidate { cursor } => {
                println!("{:#?}", cursor);
            }
        };
    });
}
