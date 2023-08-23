use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use starknet::accounts::{Account, Call, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, EmittedEvent, EventFilter, FieldElement, MsgToL1};
use starknet::core::utils::starknet_keccak;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{AnyProvider, JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use starknet_api::core::{ContractAddress, EntryPointSelector, Nonce};
use starknet_api::hash::{self, StarkFelt};
use starknet_api::stark_felt;
use starknet_api::transaction::{
    Calldata, Fee, L1HandlerTransaction as ApiL1HandlerTransaction, TransactionHash,
    TransactionVersion,
};
use url::Url;

use crate::backend::storage::transaction::L1HandlerTransaction;
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
        let provider = AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(
            Url::parse(&config.rpc_url)?,
        )));

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

    /// Fetches events for the given blocks range.
    pub async fn fetch_events(
        &self,
        from_block: BlockId,
        to_block: BlockId,
    ) -> Result<HashMap<u64, Vec<EmittedEvent>>> {
        tracing::trace!("Fetching blocks {:?} - {:?}.", from_block, to_block);

        let mut events: HashMap<u64, Vec<EmittedEvent>> = HashMap::new();

        let filter = EventFilter {
            from_block: Some(from_block),
            to_block: Some(to_block),
            address: Some(self.messaging_contract_address),
            // TODO: this might come from the configuration actually.
            keys: None,
        };

        // TODO: this chunk_size may also come from configuration?
        let chunk_size = 200;
        let mut continuation_token: Option<String> = None;

        loop {
            let event_page =
                self.provider.get_events(filter.clone(), continuation_token, chunk_size).await?;

            event_page.events.iter().for_each(|e| {
                events
                    .entry(e.block_number)
                    .and_modify(|v| v.push(e.clone()))
                    .or_insert(vec![e.clone()]);
            });

            continuation_token = event_page.continuation_token;

            if continuation_token.is_none() {
                break;
            }
        }

        Ok(events)
    }

    /// Sends an invoke TX on starknet.
    pub async fn send_invoke_tx(&self, calls: Vec<Call>) -> Result<()> {
        let signer = Arc::new(&self.wallet);

        let mut account = SingleOwnerAccount::new(
            &self.provider,
            signer,
            self.sender_account_address,
            self.chain_id,
        );

        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        // TODO: make maximum fee configurable?
        let execution = account.execute(calls).fee_estimate_multiplier(1.5f64);
        let estimated_fee = (execution.estimate_fee().await?.overall_fee) * 3 / 2;
        let _tx = execution.max_fee(estimated_fee.into()).send().await?;

        // TODO: output the TX hash?

        Ok(())
    }
}

#[async_trait]
impl Messenger for StarknetMessenger {
    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
    ) -> MessengerResult<(u64, Vec<L1HandlerTransaction>)> {
        let chain_latest_block: u64 =
            self.provider.block_number().await.map_err(|_| MessengerError::SendError).unwrap();

        // +1 as the from_block counts as 1 block fetched.
        let to_block = if from_block + max_blocks + 1 < chain_latest_block {
            from_block + max_blocks
        } else {
            chain_latest_block
        };

        let mut l1_handler_txs: Vec<L1HandlerTransaction> = vec![];

        self.fetch_events(BlockId::Number(from_block), BlockId::Number(to_block))
            .await
            .map_err(|_| MessengerError::SendError)
            .unwrap()
            .iter()
            .for_each(|(block_number, block_events)| {
                tracing::debug!(
                    "Converting events of block {} into L1HandlerTx ({} events)",
                    block_number,
                    block_events.len(),
                );

                block_events.iter().for_each(|e| {
                    if let Ok(tx) = l1_handler_tx_from_event(e) {
                        l1_handler_txs.push(tx)
                    }
                })
            });

        Ok((to_block, l1_handler_txs))
    }

    async fn settle_messages(&self, messages: &[MsgToL1]) -> MessengerResult<Vec<String>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let mut hashes: Vec<FieldElement> = vec![];
        let mut calls: Vec<Call> = vec![];

        for m in messages {
            if m.to_address == FieldElement::ZERO {
                // TODO: check payload len. Must be at least 2 felts long to decode
                // to_address and selector.

                // If it's execute -> no hash computed, only the content is taken to directly
                // build the calldata.
                // For now, if the to_address is 0, it is considered as an execute,
                // and the payload must contains [0] = to_address on starknet, [1] = selector.
                // TODO: check which special address can be taken instead of 0.
                // TODO: if this feature is considered nice by dojo team -> do a macro
                //       for the user using something like `execute_on_starknet!(...)`
                // and abstract the serialization of to_address and selector into the payload.

                calls.push(Call {
                    to: m.payload[0],
                    selector: m.payload[1],
                    calldata: m.payload[2..].to_vec(),
                });
            } else {
                let mut buf: Vec<u8> = vec![];
                buf.extend(m.from_address.to_bytes_be());
                buf.extend(m.to_address.to_bytes_be());
                buf.extend(FieldElement::from(m.payload.len()).to_bytes_be());
                for p in &m.payload {
                    buf.extend(p.to_bytes_be());
                }

                hashes.push(starknet_keccak(&buf));
            }
        }

        if !calls.is_empty() {
            match self.send_invoke_tx(calls).await {
                Ok(_) => {
                    // TODO: need to trace something here?
                }
                Err(e) => {
                    tracing::error!("Error sending invoke tx: {:?}", e);
                    return Err(MessengerError::SendError);
                }
            }
        }

        Ok(hashes.iter().map(|h| format!("{:#x}", h)).collect())
    }
}

fn l1_handler_tx_from_event(event: &EmittedEvent) -> Result<L1HandlerTransaction> {
    // TODO: replace by the topic in the filter instead of having error here.
    if event.keys[0]
        != FieldElement::from_hex_be(
            "0xd4b578bb2844b25d079c94a3e311d0327f3d260aa13ac72a7ef70212a08d8e",
        )
        .unwrap()
    {
        tracing::debug!(
            "Event with key {:?} can't be converted into L1HandlerTransaction",
            event.keys[0],
        );
        return Err(MessengerError::GatherError.into());
    }

    // See contrat appchain_messaging.cairo for MessageSentToAppchain event.
    let from_address = event.keys[2];
    let to_address = event.keys[3];
    let selector = event.data[0];
    let nonce = event.data[1];

    // Payload starts at data[2].
    let mut calldata_vec: Vec<StarkFelt> = vec![from_address.into()];
    for p in &event.data[2..] {
        calldata_vec.push((*p).into());
    }

    let calldata = Calldata(calldata_vec.into());

    let tx_hash = hash::pedersen_hash(&nonce.into(), &to_address.into());

    let tx = L1HandlerTransaction {
        inner: ApiL1HandlerTransaction {
            transaction_hash: TransactionHash(tx_hash),
            version: TransactionVersion(stark_felt!(1_u32)),
            nonce: Nonce(nonce.into()),
            contract_address: ContractAddress::try_from(<FieldElement as Into<StarkFelt>>::into(
                to_address,
            ))
            .unwrap(),
            entry_point_selector: EntryPointSelector(selector.into()),
            calldata,
        },
        // TODO: fee is missing in the event...!
        paid_fee_on_l1: Fee(30000_u128),
    };

    Ok(tx)
}
