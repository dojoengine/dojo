// TODO: just a placeholder for now, remove until we have a dedicated class for building node
// components
#[deprecated = "In the process of removal"]
#[derive(Debug, Default)]
pub struct SequencerConfig {
    pub block_time: Option<u64>,
    pub no_mining: bool,
    pub messaging: Option<crate::service::messaging::MessagingConfig>,
}
