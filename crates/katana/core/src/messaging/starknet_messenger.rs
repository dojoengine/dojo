use async_trait::async_trait;

use blockifier::transaction::transaction_execution::Transaction;
use starknet::core::types::MsgToL1;

use anyhow::{anyhow, Result};
use starknet::{
    accounts::{Account, Call, SingleOwnerAccount},
    core::{types::FieldElement, types::*},
    providers::{jsonrpc::HttpTransport, AnyProvider, JsonRpcClient, Provider},
    signers::{LocalWallet, SigningKey},
};
use std::collections::HashMap;
use url::Url;

use crate::messaging::{Messenger, MessengerError, MessengerResult};
use crate::sequencer::SequencerMessagingConfig;

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
    async fn gather_messages(&self, from_block: u64, max_blocks: u64) -> MessengerResult<(u64, Vec<Transaction>)> {
        Ok((0, vec![]))
    }

    async fn settle_messages(&self, messages: &Vec<MsgToL1>) -> MessengerResult<()> {
        Ok(())
    }

    async fn execute_messages(&self, messages: &Vec<MsgToL1>) -> MessengerResult<()> {
        Ok(())
    }
}
