use std::sync::Mutex;

use crate::{block_context::Base, state::DictStateReader};
use blockifier::{block_context::BlockContext, state::cached_state::CachedState};

pub struct KatanaSequencer {
    pub block_context: BlockContext,
    pub state: Mutex<CachedState<DictStateReader>>,
}

impl KatanaSequencer {
    pub fn new() -> Self {
        Self {
            block_context: BlockContext::base(),
            state: Mutex::new(CachedState::new(DictStateReader::new())),
        }
    }
}

impl Default for KatanaSequencer {
    fn default() -> Self {
        Self::new()
    }
}
