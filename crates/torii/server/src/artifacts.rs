use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::sync::broadcast::Receiver;
use warp::Filter;

pub async fn new(
    mut shutdown_rx: Receiver<()>,
    static_dir: PathBuf,
) -> Result<(SocketAddr, impl Future<Output = ()> + 'static), std::io::Error> {
    let routes = warp::path("static").and(warp::fs::dir(static_dir));

    Ok(warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async move {
        shutdown_rx.recv().await.ok();
    }))
}
