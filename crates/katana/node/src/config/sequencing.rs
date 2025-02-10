use katana_executor::BlockLimits;

/// Configurations related to block production.
#[derive(Debug, Clone, Default)]
pub struct SequencingConfig {
    /// The time in milliseconds for a block to be produced.
    pub block_time: Option<u64>,

    /// Disable automatic block production.
    ///
    /// Allowing block to only be produced manually.
    pub no_mining: bool,

    /// The maximum number of Cairo steps in a block.
    //
    /// The block will automatically be closed when the accumulated Cairo steps across all the
    /// transactions has reached this limit.
    ///
    /// NOTE: This only affect interval block production.
    ///
    /// See <https://docs.starknet.io/chain-info/#current_limits>.
    pub block_cairo_steps_limit: Option<u64>,
}

impl SequencingConfig {
    pub fn block_limits(&self) -> BlockLimits {
        BlockLimits { cairo_steps: self.block_cairo_steps_limit.unwrap_or(u64::MAX) }
    }
}
