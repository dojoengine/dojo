use apibara_core::node::v1alpha2::DataFinality;
use apibara_core::starknet::v1alpha2::{Block, FieldElement, Filter, HeaderFilter};
use apibara_sdk::{ClientBuilder, Configuration, DataMessage};
use bevy_app::{App, Plugin};
use bevy_core::TaskPoolPlugin;
use bevy_ecs::component::Component;
use bevy_ecs::system::{Query, ResMut};
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use tokio_stream::StreamExt;

pub struct IndexerPlugin;

impl Plugin for IndexerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(TaskPoolPlugin::default())
            .add_plugin(TokioTasksPlugin::default())
            .add_startup_system(setup)
            .add_system(log_message);
    }
}

fn setup(runtime: ResMut<TokioTasksRuntime>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DataMessage<Block>>(1);

    runtime.spawn_background_task(|_ctx| async move {
        let configuration = Configuration::<Filter>::default()
            .with_finality(DataFinality::DataStatusAccepted)
            .with_batch_size(10)
            .with_filter(build_filter);

        if let Ok(uri) = "https://mainnet.starknet.a5a.ch".parse() {
            let (mut data_stream, data_client) =
                ClientBuilder::<Filter, Block>::default().connect(uri).await.unwrap();

            data_client.send(configuration).await.unwrap();

            while let Some(message) = data_stream.try_next().await.unwrap() {
                if let Err(e) = tx.try_send(message) {
                    println!("Failed to send message: {e}");
                }
            }
        } else {
            println!("Failed to parse uri");
        }
    });

    runtime.spawn_background_task(|mut ctx| async move {
        while let Some(message) = rx.recv().await {
            ctx.run_on_main_thread(move |ctx| {
                ctx.world.spawn(IndexerMessage(message));
            })
            .await;
        }
    });
}

fn log_message(query: Query<&IndexerMessage>) {
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

#[derive(Component)]
struct IndexerMessage(DataMessage<Block>);
