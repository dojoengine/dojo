use std::collections::HashMap;
use std::str::FromStr;

use anyhow::bail;
use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::trace::{CallInfo, EntryPointType, TxExecInfo};
use katana_primitives::transaction::L1HandlerTx;
use katana_primitives::utils::transaction::compute_l1_message_hash;
use serde::{ser::SerializeSeq, ser::Serializer, Deserializer};
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/snos_output.cairo#L19-L20
/// With the new state root computed by the prover.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct ProgramInput {
    pub prev_state_root: FieldElement,
    pub block_number: u64,
    pub block_hash: FieldElement,
    pub config_hash: FieldElement,
    #[serde(serialize_with = "MessageToStarknet::serialize_message_to_starknet")]
    #[serde(deserialize_with = "MessageToStarknet::deserialize_message_to_starknet")]
    pub message_to_starknet_segment: Vec<MessageToStarknet>,
    #[serde(serialize_with = "MessageToAppchain::serialize_message_to_appchain")]
    #[serde(deserialize_with = "MessageToAppchain::deserialize_message_to_appchain")]
    pub message_to_appchain_segment: Vec<MessageToAppchain>,
    #[serde(flatten)]
    pub state_updates: StateUpdates,
    #[serde(serialize_with = "serialize_world_da")]
    pub world_da: Option<Vec<FieldElement>>,
}

fn serialize_world_da<S>(
    element: &Option<Vec<FieldElement>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(da) = element {
        let mut seq = serializer.serialize_seq(Some(da.len()))?;

        for d in da {
            let decimal = d.to_big_decimal(0); // Convert with no decimal places
            let num = decimal.to_string();
            seq.serialize_element(&num)?;
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
    transactions: &[(TxHash, &L1HandlerTx)],
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
            let tx = transactions.iter().find(|tx| tx.0 == t.hash).expect("qed; tx must exist");
            Some((tx.1, calls))
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

impl ProgramInput {
    /// Extracts the storage updates for the given world, and flattens them into a single vector
    /// that represent the serialized DA. The length is not included as the array contains
    /// serialiazed struct with two members: key and value.
    /// TODO: migrate to cainome + simple rust vec for better devX in the future.
    pub fn fill_da(&mut self, world: FieldElement) {
        let updates = self
            .state_updates
            .storage_updates
            .get(&ContractAddress::from(world))
            .unwrap_or(&std::collections::HashMap::new())
            .iter()
            .map(|(k, v)| vec![*k, *v])
            .flatten()
            .collect::<Vec<_>>();

        self.world_da = Some(updates);
    }

    pub fn combine(mut self, other: ProgramInput) -> anyhow::Result<ProgramInput> {
        self.message_to_appchain_segment.extend(other.message_to_appchain_segment);
        self.message_to_starknet_segment.extend(other.message_to_starknet_segment);

        // the later state should overwrite the previous one.
        other.state_updates.contract_updates.into_iter().for_each(|(k, v)| {
            self.state_updates.contract_updates.insert(k, v);
        });
        other.state_updates.declared_classes.into_iter().for_each(|(k, v)| {
            self.state_updates.declared_classes.insert(k, v);
        });
        other.state_updates.nonce_updates.into_iter().for_each(|(k, v)| {
            self.state_updates.nonce_updates.insert(k, v);
        });
        other.state_updates.storage_updates.into_iter().for_each(|(c, h)| {
            h.into_iter().for_each(|(k, v)| {
                self.state_updates
                    .storage_updates
                    .entry(c)
                    .or_insert_with(HashMap::new)
                    .insert(k, v);
            });
        });

        if self.world_da.is_none() || other.world_da.is_none() {
            bail!("Both world_da must be present to combine them");
        }

        let mut world_da = self.world_da.unwrap_or_default();
        for later in other.world_da.unwrap_or_default().chunks(2) {
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
            block_number: other.block_number,
            block_hash: other.block_hash,
            config_hash: self.config_hash,
            message_to_appchain_segment: self.message_to_appchain_segment,
            message_to_starknet_segment: self.message_to_starknet_segment,
            state_updates: self.state_updates,
            world_da: Some(world_da),
        })
    }

    pub fn da_as_calldata(&self, world: FieldElement) -> Vec<FieldElement> {
        let updates = self
            .state_updates
            .storage_updates
            .get(&ContractAddress::from(world))
            .unwrap_or(&std::collections::HashMap::new())
            .iter()
            .flat_map(|(k, v)| vec![*k, *v])
            .collect::<Vec<_>>();

        updates
    }
}

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/messaging/output_process.cairo#L16
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct MessageToStarknet {
    pub from_address: ContractAddress,
    pub to_address: ContractAddress,
    pub payload: Vec<FieldElement>,
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
                let decimal = field_element.to_big_decimal(0); // Assuming no decimal places for simplicity
                let num = decimal.to_string();
                seq.serialize_element(&num)?;
            }
        }
        seq.end()
    }

    pub fn serialize(&self) -> anyhow::Result<Vec<FieldElement>> {
        let mut result = vec![*self.from_address, *self.to_address];
        result.push(FieldElement::from(self.payload.len()));
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

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a flat list of integers for MessageToStarknet")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let mut messages = Vec::new();
                while let Some(from_address) = seq
                    .next_element::<&str>()?
                    .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                {
                    let to_address = seq
                        .next_element::<&str>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let payload_length_str = seq.next_element::<&str>()?.unwrap_or_default();
                    let payload_length: usize = payload_length_str.parse().unwrap_or_default();
                    let mut payload = Vec::new();
                    for _ in 0..payload_length {
                        if let Some(element) = seq
                            .next_element::<&str>()?
                            .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct MessageToAppchain {
    pub from_address: ContractAddress,
    pub to_address: ContractAddress,
    pub nonce: FieldElement,
    pub selector: FieldElement,
    pub payload: Vec<FieldElement>,
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
                let decimal = field_element.to_big_decimal(0); // Assuming no decimal places for simplicity
                let num = decimal.to_string();
                seq.serialize_element(&num)?;
            }
        }
        seq.end()
    }

    pub fn serialize(&self) -> anyhow::Result<Vec<FieldElement>> {
        let mut result = vec![*self.from_address, *self.to_address, self.nonce, self.selector];
        result.push(FieldElement::from(self.payload.len()));
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

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a flat list of integers for MessageToAppchain")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let mut messages = Vec::new();
                while let Some(from_address) = seq
                    .next_element::<&str>()?
                    .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                {
                    let to_address = seq
                        .next_element::<&str>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let nonce = seq
                        .next_element::<&str>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let selector = seq
                        .next_element::<&str>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let payload_length_str = seq.next_element::<&str>()?.unwrap_or_default();
                    let payload_length: usize = payload_length_str.parse().unwrap_or_default();
                    let mut payload = Vec::new();
                    for _ in 0..payload_length {
                        if let Some(element) = seq
                            .next_element::<&str>()?
                            .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
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

#[test]
fn test_deserialize_input() -> anyhow::Result<()> {
    use std::str::FromStr;

    let input = r#"{
        "prev_state_root":"101", 
        "block_number":102, 
        "block_hash":"103", 
        "config_hash":"104", 
        "message_to_starknet_segment":["105","106","1","1"], 
        "message_to_appchain_segment":["108","109","110","111","1","112"],
        "storage_updates":{
            "42": {
                "2010": "1200",
                "2012": "1300"
            }
        },
        "nonce_updates":{
            "1111": "22222",
            "1116": "22223"
        },
        "contract_updates":{
            "3": "437267489"
        },
        "declared_classes":{
            "1234": "12345"
        }
    }"#;

    let mut expected = ProgramInput {
        prev_state_root: FieldElement::from_str("101")?,
        block_number: 102,
        block_hash: FieldElement::from_str("103")?,
        config_hash: FieldElement::from_str("104")?,
        message_to_starknet_segment: vec![MessageToStarknet {
            from_address: ContractAddress::from(FieldElement::from_str("105")?),
            to_address: ContractAddress::from(FieldElement::from_str("106")?),
            payload: vec![FieldElement::from_str("1")?],
        }],
        message_to_appchain_segment: vec![MessageToAppchain {
            from_address: ContractAddress::from(FieldElement::from_str("108")?),
            to_address: ContractAddress::from(FieldElement::from_str("109")?),
            nonce: FieldElement::from_str("110")?,
            selector: FieldElement::from_str("111")?,
            payload: vec![FieldElement::from_str("112")?],
        }],
        state_updates: StateUpdates {
            storage_updates: vec![(
                ContractAddress::from(FieldElement::from_str("42")?),
                vec![
                    (FieldElement::from_str("2010")?, FieldElement::from_str("1200")?),
                    (FieldElement::from_str("2012")?, FieldElement::from_str("1300")?),
                ]
                .into_iter()
                .collect(),
            )]
            .into_iter()
            .collect(),

            nonce_updates: vec![
                (
                    ContractAddress::from(FieldElement::from_str("1111")?),
                    FieldElement::from_str("22222")?,
                ),
                (
                    ContractAddress::from(FieldElement::from_str("1116")?),
                    FieldElement::from_str("22223")?,
                ),
            ]
            .into_iter()
            .collect(),

            contract_updates: vec![(
                ContractAddress::from(FieldElement::from_str("3")?),
                FieldElement::from_str("437267489")?,
            )]
            .into_iter()
            .collect(),

            declared_classes: vec![(
                FieldElement::from_str("1234")?,
                FieldElement::from_str("12345")?,
            )]
            .into_iter()
            .collect(),
        },
        world_da: None,
    };
    let mut deserialized = serde_json::from_str::<ProgramInput>(input)?;
    assert_eq!(expected, deserialized);

    deserialized.fill_da(FieldElement::from_str("42")?);
    expected.world_da = Some(vec![
        FieldElement::from_str("2010")?,
        FieldElement::from_str("1200")?,
        FieldElement::from_str("2012")?,
        FieldElement::from_str("1300")?,
    ]);

    Ok(())
}

#[test]
fn test_serialize_input() -> anyhow::Result<()> {
    use std::str::FromStr;

    let input = ProgramInput {
        prev_state_root: FieldElement::from_str("101")?,
        block_number: 102,
        block_hash: FieldElement::from_str("103")?,
        config_hash: FieldElement::from_str("104")?,
        message_to_starknet_segment: vec![MessageToStarknet {
            from_address: ContractAddress::from(FieldElement::from_str("105")?),
            to_address: ContractAddress::from(FieldElement::from_str("106")?),
            payload: vec![FieldElement::from_str("1")?],
        }],
        message_to_appchain_segment: vec![MessageToAppchain {
            from_address: ContractAddress::from(FieldElement::from_str("108")?),
            to_address: ContractAddress::from(FieldElement::from_str("109")?),
            nonce: FieldElement::from_str("110")?,
            selector: FieldElement::from_str("111")?,
            payload: vec![FieldElement::from_str("112")?],
        }],
        state_updates: StateUpdates {
            storage_updates: vec![(
                ContractAddress::from(FieldElement::from_str("42")?),
                vec![
                    (FieldElement::from_str("2010")?, FieldElement::from_str("1200")?),
                    (FieldElement::from_str("2012")?, FieldElement::from_str("1300")?),
                ]
                .into_iter()
                .collect(),
            )]
            .into_iter()
            .collect(),

            nonce_updates: vec![
                (
                    ContractAddress::from(FieldElement::from_str("1111")?),
                    FieldElement::from_str("22222")?,
                ),
                (
                    ContractAddress::from(FieldElement::from_str("1116")?),
                    FieldElement::from_str("22223")?,
                ),
            ]
            .into_iter()
            .collect(),

            contract_updates: vec![(
                ContractAddress::from(FieldElement::from_str("3")?),
                FieldElement::from_str("437267489")?,
            )]
            .into_iter()
            .collect(),

            declared_classes: vec![(
                FieldElement::from_str("1234")?,
                FieldElement::from_str("12345")?,
            )]
            .into_iter()
            .collect(),
        },
        world_da: Some(vec![
            FieldElement::from_str("2010")?,
            FieldElement::from_str("1200")?,
            FieldElement::from_str("2012")?,
            FieldElement::from_str("1300")?,
        ]),
    };

    let serialized = serde_json::to_string::<ProgramInput>(&input.clone())?;
    let deserialized = serde_json::from_str::<ProgramInput>(&serialized)?;
    assert_eq!(input, deserialized);

    Ok(())
}
