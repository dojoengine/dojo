use async_trait::async_trait;

use starknet::core::types::MsgToL1;

use anyhow::{Result};
use starknet::{
    core::{types::FieldElement},
    providers::{jsonrpc::HttpTransport, AnyProvider, JsonRpcClient, Provider},
    signers::{LocalWallet, SigningKey},
};

use url::Url;

use crate::messaging::{Messenger, MessengerResult};
use crate::sequencer::SequencerMessagingConfig;
use crate::backend::storage::transaction::L1HandlerTransaction;

///
pub struct StarknetMessenger {
    chain_id: FieldElement,
    provider: AnyProvider,
    wallet: LocalWallet,
    sender_account_address: FieldElement,
    messaging_contract_address: FieldElement,
}

impl StarknetMessenger {
    pub async fn new(config: SequencerMessagingConfig) -> Result<StarknetMessenger> {
        let provider = AnyProvider::JsonRpcHttp(
            JsonRpcClient::new(HttpTransport::new(Url::parse(&config.rpc_url)?)));

        let private_key = FieldElement::from_hex_be(&config.private_key)?;
        let key = SigningKey::from_secret_scalar(private_key);
        let wallet = LocalWallet::from_signing_key(key);

        let chain_id = provider.chain_id().await?;

        let sender_account_address = FieldElement::from_hex_be(&config.sender_address)?;

        let messaging_contract_address = FieldElement::from_hex_be(&config.contract_address)?;

        Ok(StarknetMessenger {
            chain_id,
            provider,
            wallet,
            sender_account_address,
            messaging_contract_address,
        })
    }

}

#[async_trait]
impl Messenger for StarknetMessenger {
    async fn gather_messages(&self, _from_block: u64, _max_blocks: u64) -> MessengerResult<(u64, Vec<L1HandlerTransaction>)> {
        Ok((0, vec![]))
    }

    async fn settle_messages(&self, _messages: &Vec<MsgToL1>) -> MessengerResult<Vec<String>> {
        Ok(vec![])
    }
}
