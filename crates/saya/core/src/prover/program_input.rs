use std::str::FromStr;

use anyhow::bail;
use bigdecimal::BigDecimal;
use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::trace::{CallInfo, EntryPointType};
use katana_primitives::transaction::{L1HandlerTx, TxHash};
use katana_rpc_types::trace::TxExecutionInfo;
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Deserializer, Serialize};
use starknet::core::types::Call;
use starknet::core::types::Felt;

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/snos_output.cairo#L19-L20
/// With the new state root computed by the prover.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ProgramInput {
    pub prev_state_root: Felt,
    pub block_number: u64,
    pub block_hash: Felt,
    pub config_hash: Felt,
    #[serde(serialize_with = "MessageToStarknet::serialize_message_to_starknet")]
    #[serde(deserialize_with = "MessageToStarknet::deserialize_message_to_starknet")]
    pub message_to_starknet_segment: Vec<MessageToStarknet>,
    #[serde(serialize_with = "MessageToAppchain::serialize_message_to_appchain")]
    #[serde(deserialize_with = "MessageToAppchain::deserialize_message_to_appchain")]
    pub message_to_appchain_segment: Vec<MessageToAppchain>,
    #[serde(flatten)]
    pub state_updates: StateUpdates,
    #[serde(serialize_with = "serialize_world_da")]
    pub world_da: Option<Vec<Felt>>,
}

fn serialize_world_da<S>(element: &Option<Vec<Felt>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(da) = element {
        let mut seq = serializer.serialize_seq(Some(da.len()))?;

        for d in da {
            // let decimal: BigDecimal = d.to_bigint().into(); // Convert with no decimal places
            // let num = decimal.to_string();
            // seq.serialize_element(&num)?;
            seq.serialize_element(&d)?;
        }

        seq.end()
    } else {
        Err(serde::ser::Error::custom("Compute `world_da` first"))
    }
}

fn get_messages_recursively(info: &CallInfo) -> Vec<MessageToStarknet> {
    let mut messages = vec![];

    // By default, `from_address` must correspond to the contract address that
    // is sending the message. In the case of library calls, `code_address` is `None`,
    // we then use the `caller_address` instead (which can also be an account).
    let from_address =
        if let Some(code_address) = info.code_address { code_address } else { info.caller_address };

    messages.extend(info.l2_to_l1_messages.iter().map(|m| MessageToStarknet {
        from_address,
        to_address: ContractAddress::from(m.to_address),
        payload: m.payload.clone(),
    }));

    info.inner_calls.iter().for_each(|call| {
        messages.extend(get_messages_recursively(call));
    });

    messages
}

pub fn extract_messages(
    exec_infos: &[TxExecutionInfo],
    l1_transactions: &[(TxHash, &L1HandlerTx)],
) -> (Vec<MessageToStarknet>, Vec<MessageToAppchain>) {
    // extract messages to starknet (ie l2 -> l1)
    let message_to_starknet_segment = exec_infos
        .iter()
        .flat_map(|t| t.trace.execute_call_info.iter().chain(t.trace.validate_call_info.iter()).chain(t.trace.fee_transfer_call_info.iter())) // Take into account both validate and execute calls.
        .flat_map(get_messages_recursively)
        .collect();

    // extract messages to appchain (ie l1 -> l2)
    let message_to_appchain_segment = {
        // get the call infos from the trace and the corresponding tx hash
        let calls = exec_infos.iter().filter_map(|t| {
            let calls = t.trace.execute_call_info.as_ref()?;
            // Not present if not a l1 handler tx.
            l1_transactions.iter().find(|tx| tx.0 == t.hash).map(|(_, tx)| (tx, calls))
        });

        // filter only the l1 handler tx
        let l1_handlers = calls.filter(|(_, c)| c.entry_point_type == EntryPointType::L1Handler);

        // build messages
        l1_handlers
            .map(|(t, c)| MessageToAppchain {
                nonce: t.nonce,
                payload: c.calldata.clone(),
                from_address: c.caller_address,
                to_address: c.contract_address,
                selector: c.entry_point_selector,
            })
            .collect()
    };

    (message_to_starknet_segment, message_to_appchain_segment)
}

pub fn extract_execute_calls(exec_infos: &[TxExecutionInfo]) -> Vec<Call> {
    // Ignoring the inner calls at this point.
    exec_infos
        .iter()
        .filter_map(|t| t.trace.execute_call_info.clone())
        .map(|c| Call {
            to: c.contract_address.into(),
            selector: c.entry_point_selector,
            calldata: c.calldata,
        })
        .collect()
}

impl ProgramInput {
    /// Extracts the storage updates for the given world, and flattens them into a single vector
    /// that represent the serialized DA. The length is not included as the array contains
    /// serialiazed struct with two members: key and value.
    /// TODO: migrate to cainome + simple rust vec for better devX in the future.
    pub fn fill_da(&mut self, world: Felt) {
        let updates = self
            .state_updates
            .storage_updates
            .get(&ContractAddress::from(world))
            .unwrap_or(&std::collections::BTreeMap::new())
            .iter()
            .flat_map(|(k, v)| vec![*k, *v])
            .collect::<Vec<_>>();

        self.world_da = Some(updates);
    }

    pub fn combine(mut self, latter: ProgramInput) -> anyhow::Result<ProgramInput> {
        self.message_to_appchain_segment.extend(latter.message_to_appchain_segment);
        self.message_to_starknet_segment.extend(latter.message_to_starknet_segment);

        // the later state should overwrite the previous one.
        latter.state_updates.deployed_contracts.into_iter().for_each(|(k, v)| {
            self.state_updates.deployed_contracts.insert(k, v);
        });
        latter.state_updates.declared_classes.into_iter().for_each(|(k, v)| {
            self.state_updates.declared_classes.insert(k, v);
        });
        latter.state_updates.nonce_updates.into_iter().for_each(|(k, v)| {
            self.state_updates.nonce_updates.insert(k, v);
        });
        latter.state_updates.storage_updates.into_iter().for_each(|(c, h)| {
            h.into_iter().for_each(|(k, v)| {
                self.state_updates.storage_updates.entry(c).or_default().insert(k, v);
            });
        });

        if self.world_da.is_none() || latter.world_da.is_none() {
            bail!("Both world_da must be present to combine them");
        }

        let mut world_da = self.world_da.unwrap_or_default();
        for later in latter.world_da.unwrap_or_default().chunks(2) {
            let mut replaced = false;
            for earlier in world_da.chunks_mut(2) {
                if later[0] == earlier[0] {
                    earlier[1] = later[1];
                    replaced = true;
                    continue;
                }
            }

            if !replaced {
                world_da.extend(later)
            }
        }

        // The block number is the one from the last block.
        Ok(ProgramInput {
            prev_state_root: self.prev_state_root,
            block_number: latter.block_number,
            block_hash: latter.block_hash,
            config_hash: self.config_hash,
            message_to_appchain_segment: self.message_to_appchain_segment,
            message_to_starknet_segment: self.message_to_starknet_segment,
            state_updates: self.state_updates,
            world_da: Some(world_da),
        })
    }

    pub fn da_as_calldata(&self, world: Felt) -> Vec<Felt> {
        let updates = self
            .state_updates
            .storage_updates
            .get(&ContractAddress::from(world))
            .unwrap_or(&std::collections::BTreeMap::new())
            .iter()
            .flat_map(|(k, v)| vec![*k, *v])
            .collect::<Vec<_>>();

        updates
    }
    //TODO: change to use cainome/serde_felt
    fn serialize_to_prover_args(&self) -> Vec<Felt> {
        let mut out = vec![
            self.prev_state_root,
            Felt::from(self.block_number),
            self.block_hash,
            self.config_hash,
        ];

        out.push(Felt::from(self.state_updates.nonce_updates.len()));
        for (k, v) in &self.state_updates.nonce_updates {
            out.push(**k);
            out.push(*v);
        }

        out.push(Felt::from(self.state_updates.storage_updates.len()));
        for (c, h) in &self.state_updates.storage_updates {
            out.push(**c);
            out.push(Felt::from(h.len()));
            for (k, v) in h {
                out.push(*k);
                out.push(*v);
            }
        }

        out.push(Felt::from(self.state_updates.deployed_contracts.len()));
        for (k, v) in &self.state_updates.deployed_contracts {
            out.push(**k);
            out.push(*v);
        }

        out.push(Felt::from(self.state_updates.declared_classes.len()));
        for (k, v) in &self.state_updates.declared_classes {
            out.push(*k);
            out.push(*v);
        }

        let starknet_messages = self
            .message_to_starknet_segment
            .iter()
            .flat_map(|m| m.serialize().unwrap())
            .collect::<Vec<_>>();
        out.push(Felt::from(starknet_messages.len()));
        out.extend(starknet_messages);

        let appchain_messages = self
            .message_to_appchain_segment
            .iter()
            .flat_map(|m| m.serialize().unwrap())
            .collect::<Vec<_>>();

        out.push(Felt::from(appchain_messages.len()));
        out.extend(appchain_messages);

        out.push(Felt::from(self.world_da.as_ref().unwrap().len() / 2));
        out.extend(self.world_da.as_ref().unwrap().iter().cloned());

        out.push(Felt::from(0u64)); // Proofs

        out
    }

    pub fn prepare_differ_args(inputs: Vec<ProgramInput>) -> String {
        let serialized =
            inputs.iter().flat_map(|input| input.serialize_to_prover_args()).collect::<Vec<_>>();

        let joined = serialized
            .iter()
            .map(|f| BigDecimal::from(f.to_bigint()).to_string())
            .collect::<Vec<_>>();

        format!("[{} {}]", inputs.len(), joined.join(" "))
    }
}

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/messaging/output_process.cairo#L16
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct MessageToStarknet {
    pub from_address: ContractAddress,
    pub to_address: ContractAddress,
    pub payload: Vec<Felt>,
}

impl MessageToStarknet {
    pub fn serialize_message_to_starknet<S>(
        messages: &[MessageToStarknet],
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(messages.len()))?;
        for message in messages {
            let serialized = message.serialize().unwrap();
            // Instead of adding serialized as an array, add each element individually
            for field_element in serialized {
                // let decimal: BigDecimal = field_element.to_bigint().into(); // Assuming no
                // decimal places for simplicity let num = decimal.to_string();
                // seq.serialize_element(&num)?;
                seq.serialize_element(&field_element)?;
            }
        }
        seq.end()
    }

    pub fn serialize(&self) -> anyhow::Result<Vec<Felt>> {
        let mut result = vec![*self.from_address, *self.to_address];
        result.push(Felt::from(self.payload.len()));
        result.extend(self.payload.iter().cloned());
        Ok(result)
    }

    fn deserialize_message_to_starknet<'de, D>(
        deserializer: D,
    ) -> Result<Vec<MessageToStarknet>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MessageToStarknetVisitor;

        impl<'de> serde::de::Visitor<'de> for MessageToStarknetVisitor {
            type Value = Vec<MessageToStarknet>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a flat list of integers for MessageToStarknet")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let mut messages = Vec::new();
                while let Some(from_address) = seq
                    .next_element::<String>()?
                    .map(|num| Felt::from_str(&num.to_string()).unwrap())
                {
                    let to_address = seq
                        .next_element::<String>()?
                        .map(|num| Felt::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();

                    let payload_length_str = seq.next_element::<String>()?.unwrap_or_default();
                    // TODO: for compatibility reason, the length can be in either decimal or hex
                    // format. maybe should just expect all values to be in hex format.
                    let payload_length = payload_length_str
                        .parse::<usize>()
                        .or_else(|_| {
                            usize::from_str_radix(
                                payload_length_str
                                    .strip_prefix("0x")
                                    .unwrap_or(&payload_length_str),
                                16,
                            )
                        })
                        .expect("invalid length value");

                    let mut payload = Vec::new();
                    for _ in 0..payload_length {
                        if let Some(element) = seq
                            .next_element::<String>()?
                            .map(|num| Felt::from_str(&num.to_string()).unwrap())
                        {
                            payload.push(element);
                        }
                    }
                    messages.push(MessageToStarknet {
                        from_address: ContractAddress::from(from_address),
                        to_address: ContractAddress::from(to_address),
                        payload,
                    });
                }
                Ok(messages)
            }
        }

        deserializer.deserialize_seq(MessageToStarknetVisitor)
    }
}

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/messaging/output_process.cairo#L28
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct MessageToAppchain {
    pub from_address: ContractAddress,
    pub to_address: ContractAddress,
    pub nonce: Felt,
    pub selector: Felt,
    pub payload: Vec<Felt>,
}

impl MessageToAppchain {
    pub fn serialize_message_to_appchain<S>(
        messages: &[MessageToAppchain],
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(messages.len()))?;
        for message in messages {
            let serialized = message.serialize().unwrap();
            for field_element in serialized {
                // let decimal: BigDecimal = field_element.to_bigint().into(); // Assuming no
                // decimal places for simplicity let num = decimal.to_string();
                // seq.serialize_element(&num)?;
                seq.serialize_element(&field_element)?;
            }
        }
        seq.end()
    }

    pub fn serialize(&self) -> anyhow::Result<Vec<Felt>> {
        let mut result = vec![*self.from_address, *self.to_address, self.nonce, self.selector];
        result.push(Felt::from(self.payload.len()));
        result.extend(self.payload.iter().cloned());
        Ok(result)
    }

    fn deserialize_message_to_appchain<'de, D>(
        deserializer: D,
    ) -> Result<Vec<MessageToAppchain>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MessageToAppchainVisitor;

        impl<'de> serde::de::Visitor<'de> for MessageToAppchainVisitor {
            type Value = Vec<MessageToAppchain>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a flat list of integers for MessageToAppchain")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let mut messages = Vec::new();
                while let Some(from_address) = seq
                    .next_element::<String>()?
                    .map(|num| Felt::from_str(&num.to_string()).unwrap())
                {
                    let to_address = seq
                        .next_element::<String>()?
                        .map(|num| Felt::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let nonce = seq
                        .next_element::<String>()?
                        .map(|num| Felt::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let selector = seq
                        .next_element::<String>()?
                        .map(|num| Felt::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();

                    let payload_length_str = seq.next_element::<String>()?.unwrap_or_default();
                    // TODO: for compatibility reason, the length can be in either decimal or hex
                    // format. maybe should just expect all values to be in hex format.
                    let payload_length = payload_length_str
                        .parse::<usize>()
                        .or_else(|_| {
                            usize::from_str_radix(
                                payload_length_str
                                    .strip_prefix("0x")
                                    .unwrap_or(&payload_length_str),
                                16,
                            )
                        })
                        .expect("invalid length value");

                    let mut payload = Vec::new();
                    for _ in 0..payload_length {
                        if let Some(element) = seq
                            .next_element::<String>()?
                            .map(|num| Felt::from_str(&num.to_string()).unwrap())
                        {
                            payload.push(element);
                        }
                    }
                    messages.push(MessageToAppchain {
                        from_address: ContractAddress::from(from_address),
                        to_address: ContractAddress::from(to_address),
                        nonce,
                        selector,
                        payload,
                    });
                }
                Ok(messages)
            }
        }

        deserializer.deserialize_seq(MessageToAppchainVisitor)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use katana_primitives::{address, felt};

    use super::*;

    #[test]
    fn test_deserialize_input() -> anyhow::Result<()> {
        let input = r#"{
        "prev_state_root":"0x65",
        "block_number": 102,
        "block_hash":"0x67",
        "config_hash":"0x68",
        "message_to_starknet_segment":["0x69","0x6a","0x1","0x1"],
        "message_to_appchain_segment":["0x6c","0x6d","0x6e","0x6f","0x1","0x70"],
        "storage_updates":{
            "0x2a": {
                "0x7dc": "0x514",
                "0x7da": "0x4b0"
            }
        },
        "nonce_updates":{
            "0x457": "0x56ce",
            "0x45c": "0x56cf"
        },
        "deployed_contracts":{
            "0x3": "0x1a102c21"
        },
        "declared_classes":{
            "0x4d2": "0x3039"
        },
        "deprecated_declared_classes": [],
        "replaced_classes": {}
    }"#;
        let mut expected = ProgramInput {
            prev_state_root: Felt::from_str("101")?,
            block_number: 102,
            block_hash: Felt::from_str("103")?,
            config_hash: Felt::from_str("104")?,
            message_to_starknet_segment: vec![MessageToStarknet {
                from_address: address!("105"),
                to_address: address!("106"),
                payload: vec![felt!("1")],
            }],
            message_to_appchain_segment: vec![MessageToAppchain {
                from_address: address!("108"),
                to_address: address!("109"),
                nonce: felt!("110"),
                selector: felt!("111"),
                payload: vec![Felt::from_str("112")?],
            }],
            state_updates: StateUpdates {
                storage_updates: vec![(
                    address!("42"),
                    vec![
                        (felt!("2010"), felt!("1200")),
                        (Felt::from_str("2012")?, Felt::from_str("1300")?),
                    ]
                    .into_iter()
                    .collect(),
                )]
                .into_iter()
                .collect(),

                nonce_updates: vec![
                    (address!("1111"), felt!("22222")),
                    (address!("1116"), felt!("22223")),
                ]
                .into_iter()
                .collect(),

                deployed_contracts: vec![(address!("3"), felt!("437267489"))].into_iter().collect(),

                declared_classes: vec![(Felt::from_str("1234")?, Felt::from_str("12345")?)]
                    .into_iter()
                    .collect(),

                ..Default::default()
            },
            world_da: None,
        };
        let mut deserialized = serde_json::from_str::<ProgramInput>(input)?;
        assert_eq!(expected, deserialized);

        deserialized.fill_da(Felt::from_str("42")?);
        expected.world_da = Some(vec![
            Felt::from_str("2010")?,
            Felt::from_str("1200")?,
            Felt::from_str("2012")?,
            Felt::from_str("1300")?,
        ]);

        Ok(())
    }

    #[test]
    fn test_serialize_input() -> anyhow::Result<()> {
        use std::str::FromStr;

        let input = ProgramInput {
            prev_state_root: Felt::from_str("101")?,
            block_number: 102,
            block_hash: Felt::from_str("103")?,
            config_hash: felt!("104"),
            message_to_starknet_segment: vec![MessageToStarknet {
                from_address: address!("105"),
                to_address: address!("106"),
                payload: vec![felt!("1")],
            }],
            message_to_appchain_segment: vec![MessageToAppchain {
                from_address: address!("108"),
                to_address: address!("109"),
                nonce: felt!("110"),
                selector: felt!("111"),
                payload: vec![felt!("112")],
            }],
            state_updates: StateUpdates {
                storage_updates: vec![(
                    address!("42"),
                    vec![(felt!("2010"), felt!("1200")), (felt!("2012"), felt!("1300"))]
                        .into_iter()
                        .collect(),
                )]
                .into_iter()
                .collect(),

                nonce_updates: vec![
                    (address!("1111"), felt!("22222")),
                    (address!("1116"), felt!("22223")),
                ]
                .into_iter()
                .collect(),

                deployed_contracts: vec![(address!("3"), felt!("437267489"))].into_iter().collect(),
                declared_classes: vec![(felt!("1234"), felt!("12345"))].into_iter().collect(),

                ..Default::default()
            },
            world_da: Some(vec![felt!("2010"), felt!("1200"), felt!("2012"), felt!("1300")]),
        };

        let serialized = serde_json::to_string::<ProgramInput>(&input.clone())?;
        let deserialized = serde_json::from_str::<ProgramInput>(&serialized)?;
        assert_eq!(input, deserialized);

        Ok(())
    }

    #[test]
    fn test_serialize_to_prover_args() -> anyhow::Result<()> {
        let input = r#"{
        "prev_state_root":"0x65",
        "block_number":102,
        "block_hash":"0x67",
        "config_hash":"0x68",
        "nonce_updates":{
            "0x457": "0x56ce"
        },
        "storage_updates":{
            "0x14d": {
                "0x115c": "0x22b"
            }
        },
        "deployed_contracts":{
            "0x1046a": "0x1e61"
        },
        "declared_classes":{
            "0x15b38": "0x1869f"
        },
        "deprecated_declared_classes": [],
        "replaced_classes": {},
        "message_to_starknet_segment":["0x7b","0x1c8","0x7b","0x80"],
        "message_to_appchain_segment":["0x6c","0x6d","0x6e","0x6f","0x1","0x70"]
    }"#;
        let mut input = serde_json::from_str::<ProgramInput>(input)?;
        input.fill_da(felt!("333"));

        let serialized = input.serialize_to_prover_args();

        let expected = vec![
            101, 102, 103, 104, 1, 1111, 22222, 1, 333, 1, 4444, 555, 1, 66666, 7777, 1, 88888,
            99999, 4, 123, 456, 1, 128, 6, 108, 109, 110, 111, 1, 112, 1, 4444, 555, 0u64,
        ]
        .into_iter()
        .map(Felt::from)
        .collect::<Vec<_>>();

        assert_eq!(serialized, expected);

        Ok(())
    }
}
