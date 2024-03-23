use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use alloy_network::Ethereum;
use alloy_primitives::{Address, LogData, U256};
use alloy_provider::{HttpProvider, Provider};
use alloy_rpc_types::{BlockNumberOrTag, Filter, FilterBlockOption, FilterSet, Log, Topic};
use alloy_signer_wallet::LocalWallet;
use alloy_sol_types::{sol, SolEvent};
use anyhow::Result;
use async_trait::async_trait;
use katana_primitives::chain::ChainId;
use katana_primitives::receipt::MessageToL1;
use katana_primitives::transaction::L1HandlerTx;
use katana_primitives::utils::transaction::compute_l1_message_hash;
use katana_primitives::FieldElement;
use tracing::{debug, trace, warn};

use super::{Error, MessagingConfig, Messenger, MessengerResult, LOG_TARGET};

sol! {
    #[sol(rpc, rename_all = "camelcase")]
    #[derive(serde::Serialize, serde::Deserialize)]
    StarknetMessagingLocal,
    "../primitives/contracts/messaging/solidity/IStarknetMessagingLocal_ABI.json"
}

sol! {
    #[sol(rpc)]
    contract LogMessageToL2 {
        #[derive(Debug, PartialEq)]
        event LogMessageToL2Event(
            address indexed from_address,
            uint256 indexed to_address,
            uint256 indexed selector,
            uint256[] payload,
            uint256 nonce,
            uint256 fee
        );
    }
}

pub struct EthereumMessaging {
    provider: Arc<HttpProvider<Ethereum>>,
    messaging_contract_address: Address,
}

impl EthereumMessaging {
    pub async fn new(config: MessagingConfig) -> Result<EthereumMessaging> {
        Ok(EthereumMessaging {
            provider: Arc::new(HttpProvider::<Ethereum>::new_http(reqwest::Url::parse(
                &config.rpc_url,
            )?)),
            messaging_contract_address: config.contract_address.parse::<Address>()?,
        })
    }

    /// Fetches logs in given block range and returns a `HashMap` with the list of logs mapped to
    /// their block number.
    ///
    /// There is not pagination in ethereum, and no hard limit on block range.
    /// Fetching too much block may result in RPC request error.
    /// For this reason, the caller may wisely choose the range.
    ///
    /// # Arguments
    ///
    /// * `from_block` - The first block of which logs must be fetched.
    /// * `to_block` - The last block of which logs must be fetched.
    pub async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> MessengerResult<HashMap<u64, Vec<Log>>> {
        trace!(target: LOG_TARGET, "Fetching logs for blocks {} - {}.", from_block, to_block);

        let mut block_to_logs: HashMap<u64, Vec<Log>> = HashMap::new();

        let filters = Filter {
            block_option: FilterBlockOption::Range {
                from_block: Some(BlockNumberOrTag::Number(from_block.into())),
                to_block: Some(BlockNumberOrTag::Number(to_block.into())),
            },
            address: FilterSet::<Address>::from(self.messaging_contract_address),
            topics: [
                Topic::from(
                    "0xdb80dd488acf86d17c747445b0eabb5d57c541d3bd7b6b87af987858e5066b2b"
                        .parse::<U256>()
                        .unwrap(),
                ),
                Default::default(),
                Default::default(),
                Default::default(),
            ],
        };

        self.provider
            .get_logs(&filters)
            .await?
            .into_iter()
            .filter(|log| log.block_number.is_some())
            .map(|log| {
                (
                    log.block_number
                        .unwrap()
                        .try_into()
                        .expect("Block number couldn't be converted to u64."),
                    log,
                )
            })
            .for_each(|(block_num, log)| {
                block_to_logs
                    .entry(block_num)
                    .and_modify(|v| v.push(log.clone()))
                    .or_insert(vec![log]);
            });

        Ok(block_to_logs)
    }
}

#[async_trait]
impl Messenger for EthereumMessaging {
    type MessageHash = U256;
    type MessageTransaction = L1HandlerTx;

    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
        chain_id: ChainId,
    ) -> MessengerResult<(u64, Vec<Self::MessageTransaction>)> {
        let chain_latest_block: u64 = self
            .provider
            .get_block_number()
            .await?
            .try_into()
            .expect("Can't convert latest block number into u64.");

        // +1 as the from_block counts as 1 block fetched.
        let to_block = if from_block + max_blocks + 1 < chain_latest_block {
            from_block + max_blocks
        } else {
            chain_latest_block
        };

        let mut l1_handler_txs = vec![];

        self.fetch_logs(from_block, to_block).await?.into_iter().for_each(
            |(block_number, block_logs)| {
                debug!(
                    target: LOG_TARGET,
                    "Converting logs of block {block_number} into L1HandlerTx ({} logs)",
                    block_logs.len(),
                );

                block_logs.into_iter().for_each(|log| {
                    if let Ok(tx) = l1_handler_tx_from_log(log, chain_id) {
                        l1_handler_txs.push(tx)
                    }
                })
            },
        );

        Ok((to_block, l1_handler_txs))
    }

    async fn send_messages(
        &self,
        messages: &[MessageToL1],
    ) -> MessengerResult<Vec<Self::MessageHash>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let starknet_messaging =
            StarknetMessagingLocal::new(self.messaging_contract_address, self.provider.clone());

        let hashes = parse_messages(messages);

        debug!("Sending transaction on L1 to register messages...");

        let receipt = starknet_messaging
            .addMessageHashesFromL2(hashes.clone())
            .send()
            .await
            .map_err(|_| Error::SendError)?
            .get_receipt()
            .await
            .map_err(|_| {
                warn!(target: LOG_TARGET, "No receipt for L1 transaction.");
                Error::SendError
            })?;

        trace!(
            target: LOG_TARGET,
            "Transaction sent on L1 to register {} messages: {:#x}",
            hashes.len(),
            receipt.transaction_hash,
        );

        Ok(hashes)
    }
}

fn l1_handler_tx_from_log(log: Log, chain_id: ChainId) -> MessengerResult<L1HandlerTx> {
    let parsed_log = LogMessageToL2::LogMessageToL2Event::decode_log(
        &alloy_primitives::Log::<LogData>::new(log.address, log.topics, log.data).unwrap(),
        false,
    )
    .unwrap();

    let from_address = felt_from_address(parsed_log.from_address);
    let contract_address = felt_from_u256(parsed_log.to_address);
    let entry_point_selector = felt_from_u256(parsed_log.selector);
    let nonce = felt_from_u256(parsed_log.nonce);
    let paid_fee_on_l1: u128 = parsed_log.fee.try_into().expect("Fee does not fit into u128.");

    let mut calldata = vec![from_address];
    calldata.extend(parsed_log.payload.clone().into_iter().map(felt_from_u256));

    let message_hash = compute_l1_message_hash(from_address, contract_address, &calldata);

    Ok(L1HandlerTx {
        nonce,
        calldata,
        chain_id,
        message_hash,
        paid_fee_on_l1,
        entry_point_selector,
        version: FieldElement::ZERO,
        contract_address: contract_address.into(),
    })
}

/// With Ethereum, the messages are following the conventional starknet messaging.
fn parse_messages(messages: &[MessageToL1]) -> Vec<U256> {
    messages
        .iter()
        .map(|msg| {
            U256::from_be_bytes(
                compute_l1_message_hash(msg.from_address.into(), msg.to_address, &msg.payload)
                    .into(),
            )
        })
        .collect()
}

fn felt_from_u256(v: U256) -> FieldElement {
    FieldElement::from_str(format!("{:#064x}", v).as_str()).unwrap()
}

fn felt_from_address(v: Address) -> FieldElement {
    FieldElement::from_str(format!("{:#064x}", v).as_str()).unwrap()
}

#[cfg(test)]
mod tests {

    use alloy_primitives::{Address, B256, U256};
    use katana_primitives::chain::{ChainId, NamedChainId};
    use starknet::macros::{felt, selector};

    use super::*;

    #[test]
    fn l1_handler_tx_from_log_parse_ok() {
        let from_address = "0x000000000000000000000000be3C44c09bc1a3566F3e1CA12e5AbA0fA4Ca72Be";
        let to_address = "0x039dc79e64f4bb3289240f88e0bae7d21735bef0d1a51b2bf3c4730cb16983e1";
        let selector = "0x02f15cff7b0eed8b9beb162696cf4e3e0e35fa7032af69cd1b7d2ac67a13f40f";
        let nonce = 783082_u128;
        let fee = 30000_u128;

        // Payload two values: [1, 2].
        let payload_buf = hex::decode("000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000bf2ea0000000000000000000000000000000000000000000000000000000000007530000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002").unwrap();

        let calldata = vec![
            FieldElement::from_hex_be(from_address).unwrap(),
            FieldElement::ONE,
            FieldElement::TWO,
        ];

        let expected_tx_hash =
            felt!("0x6182c63599a9638272f1ce5b5cadabece9c81c2d2b8f88ab7a294472b8fce8b");

        let log = Log {
            address: Address::from_str("0xde29d060D45901Fb19ED6C6e959EB22d8626708e").unwrap(),
            topics: vec![
                B256::from_str(
                    "0xdb80dd488acf86d17c747445b0eabb5d57c541d3bd7b6b87af987858e5066b2b",
                )
                .unwrap(),
                B256::from_str(from_address).unwrap(),
                B256::from_str(to_address).unwrap(),
                B256::from_str(selector).unwrap(),
            ],
            data: payload_buf.into(),
            ..Default::default()
        };

        // SN_GOERLI.
        let chain_id = ChainId::Named(NamedChainId::Goerli);
        let to_address = FieldElement::from_hex_be(to_address).unwrap();
        let from_address = FieldElement::from_hex_be(from_address).unwrap();

        let message_hash = compute_l1_message_hash(from_address, to_address, &calldata);

        let expected = L1HandlerTx {
            calldata,
            chain_id,
            message_hash,
            paid_fee_on_l1: fee,
            version: FieldElement::ZERO,
            nonce: FieldElement::from(nonce),
            contract_address: to_address.into(),
            entry_point_selector: FieldElement::from_hex_be(selector).unwrap(),
        };
        let tx_hash = expected.calculate_hash();

        let tx = l1_handler_tx_from_log(log, chain_id).expect("bad log format");

        assert_eq!(tx, expected);
        assert_eq!(tx_hash, expected_tx_hash);
    }

    #[test]
    fn parse_msg_to_l1() {
        let from_address = selector!("from_address");
        let to_address = selector!("to_address");
        let payload = vec![FieldElement::ONE, FieldElement::TWO];

        let messages = vec![MessageToL1 { from_address: from_address.into(), to_address, payload }];

        let hashes = parse_messages(&messages);
        assert_eq!(hashes.len(), 1);
        assert_eq!(
            hashes[0],
            U256::from_str_radix(
                "5ba1d2e131360f15e26dd4f6ff10550685611cc25f75e7950b704adb04b36162",
                16
            )
            .unwrap()
        );
    }
}
