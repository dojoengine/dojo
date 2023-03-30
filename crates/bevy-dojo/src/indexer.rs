use std::marker::PhantomData;

use apibara_sdk::{ClientBuilder, Configuration, DataMessage};
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::system::{Res, ResMut, Resource};
use bevy::log;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use prost::Message;
use tokio_stream::StreamExt;
pub use tonic::transport::Uri;

pub struct IndexerPlugin<F: Message + Default, D> {
    uri: Uri,
    config: Configuration<F>,
    _message_data: PhantomData<D>,
}

#[derive(Resource)]
struct Config<F: Message + Default, D: Message + Default> {
    uri: Uri,
    data_stream_config: Configuration<F>,
    _message_data: PhantomData<D>,
}

impl<F, D> Plugin for IndexerPlugin<F, D>
where
    F: Message + Default + 'static + Clone,
    D: Message + Default + 'static,
{
    fn build(&self, app: &mut App) {
        app.add_plugin(TokioTasksPlugin::default())
            .insert_resource(Config {
                uri: self.uri.clone(),
                data_stream_config: self.config.clone(),
                _message_data: PhantomData::<D>,
            })
            .add_startup_system(setup::<F, D>);
    }
}

impl<F, D> IndexerPlugin<F, D>
where
    F: Message + Default + 'static,
{
    pub fn new(uri: &'static str) -> Self {
        Self::new_with_config(uri, Configuration::<F>::default())
    }

    pub fn new_with_config(uri: &'static str, config: Configuration<F>) -> Self {
        Self { uri: uri.parse().unwrap(), config, _message_data: PhantomData }
    }
}

fn setup<F, D>(runtime: ResMut<'_, TokioTasksRuntime>, config: Res<'_, Config<F, D>>)
where
    F: Message + Default + 'static + Clone,
    D: Message + Default + 'static,
{
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DataMessage<D>>(1);

    let data_stream_config = config.data_stream_config.clone();
    let uri = config.uri.clone();
    runtime.spawn_background_task(|_ctx| async move {
        let (mut data_stream, data_client) =
            ClientBuilder::<F, D>::default().connect(uri).await.unwrap();

        data_client.send(data_stream_config).await.unwrap();

        while let Some(message) = data_stream.try_next().await.unwrap() {
            if let Err(e) = tx.try_send(message) {
                log::error!("Failed to send message: {e}");
            }
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

#[derive(Component)]
pub struct IndexerMessage<D: Message + Default>(pub DataMessage<D>);
