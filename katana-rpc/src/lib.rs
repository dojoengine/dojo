pub mod api;

use crate::api::{KatanaApiError, KatanaApiServer};

use jsonrpsee::{
    core::{async_trait, Error},
    server::{ServerBuilder, ServerHandle},
};

use katana_core::sequencer::Sequencer;
use std::net::SocketAddr;

pub struct KatanaRpc {
    sequencer: Sequencer,
}

impl KatanaRpc {
    pub fn new(sequencer: Sequencer) -> Self {
        Self { sequencer }
    }

    pub async fn run(self) -> Result<(SocketAddr, ServerHandle), Error> {
        let server = ServerBuilder::new()
            .build("127.0.0.1:0")
            .await
            .map_err(|_| Error::from(KatanaApiError::InternalServerError))?;

        let addr = server.local_addr()?;
        let handle = server.start(self.into_rpc())?;

        Ok((addr, handle))
    }
}

#[async_trait]
impl KatanaApiServer for KatanaRpc {
    async fn chain_id(&self) -> Result<String, Error> {
        Ok(self.sequencer.block_context.chain_id.to_string())
    }
}
