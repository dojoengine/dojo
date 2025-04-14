//! Torii binary executable.
//!
//! ## Feature Flags
//!
//! - `jemalloc`: Uses [jemallocator](https://github.com/tikv/jemallocator) as the global allocator.
//!   This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.
//! - `jemalloc-prof`: Enables [jemallocator's](https://github.com/tikv/jemallocator) heap profiling
//!   and leak detection functionality. See [jemalloc's opt.prof](https://jemalloc.net/jemalloc.3.html#opt.prof)
//!   documentation for usage details. This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.

use clap::Parser;
use cli::Cli;
use torii_runner::Runner;
use tracing_subscriber::{fmt, EnvFilter};

mod cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set the global tracing subscriber
    let filter_layer =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("torii=info"));

    let subscriber = fmt::Subscriber::builder().with_env_filter(filter_layer).finish();

    // Set the global subscriber
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set the global tracing subscriber");

    let args = Cli::parse().args.with_config_file()?;
    let runner = Runner::new(args, env!("TORII_VERSION_SPEC").to_string());
    runner.run().await?;
    Ok(())
}
