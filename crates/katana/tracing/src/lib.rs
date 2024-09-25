use tracing::Subscriber;
use tracing_subscriber::{fmt, EnvFilter};

// fn init_logging() {
//     const DEFAULT_LOG_FILTER: &str = "info,executor=trace,forking::backend=trace,server=debug,\
//                                           katana_core=trace,blockifier=off,jsonrpsee_server=off,\
//                                           hyper=off,messaging=debug,node=error";

//     let builder = fmt::Subscriber::builder().with_env_filter(
//         EnvFilter::try_from_default_env().or(EnvFilter::try_new(DEFAULT_LOG_FILTER))?,
//     );

//     let subscriber: Box<dyn Subscriber + Send + Sync> =
//         if json_log { Box::new(builder.json().finish()) } else { Box::new(builder.finish()) };

//     tracing::subscriber::set_global_default(subscriber);

//     todo!()
// }
