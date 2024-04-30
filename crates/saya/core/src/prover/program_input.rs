use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::trace::{CallInfo, EntryPointType, TxExecInfo};
use katana_primitives::transaction::L1HandlerTx;
use katana_primitives::utils::transaction::compute_l1_message_hash;
use num_traits::cast::ToPrimitive;
use serde::{de::Error as DeError, ser::SerializeSeq, ser::Serializer, Deserializer};
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use std::collections::HashMap;
use std::str::FromStr;

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/snos_output.cairo#L19-L20
/// With the new state root computed by the prover.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]

pub struct ProgramInput {
    #[serde(serialize_with = "serialize_field_element_as_u64")]
    #[serde(deserialize_with = "deserialize_field_element_from_u64")]
    pub prev_state_root: FieldElement,
    pub block_number: u64,
    #[serde(serialize_with = "serialize_field_element_as_u64")]
    #[serde(deserialize_with = "deserialize_field_element_from_u64")]
    pub block_hash: FieldElement,
    #[serde(serialize_with = "serialize_field_element_as_u64")]
    #[serde(deserialize_with = "deserialize_field_element_from_u64")]
    pub config_hash: FieldElement,
    #[serde(serialize_with = "MessageToStarknet::serialize_message_to_starknet")]
    #[serde(deserialize_with = "MessageToStarknet::deserialize_message_to_starknet")]
    pub message_to_starknet_segment: Vec<MessageToStarknet>,
    #[serde(serialize_with = "MessageToAppchain::serialize_message_to_appchain")]
    #[serde(deserialize_with = "MessageToAppchain::deserialize_message_to_appchain")]
    pub message_to_appchain_segment: Vec<MessageToAppchain>,
    #[serde(flatten)]
    pub state_updates: StateUpdates,
}

fn serialize_field_element_as_u64<S>(
    element: &FieldElement,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let decimal = element.to_big_decimal(0); // Convert with no decimal places
    let num = decimal
        .to_u64()
        .ok_or_else(|| serde::ser::Error::custom("FieldElement conversion to u64 failed"))?;
    serializer.serialize_u64(num)
}
fn deserialize_field_element_from_u64<'de, D>(deserializer: D) -> Result<FieldElement, D::Error>
where
    D: Deserializer<'de>,
{
    let num = u64::deserialize(deserializer)?;
    FieldElement::from_dec_str(&num.to_string()).map_err(DeError::custom)
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
    exec_infos: &[TxExecInfo],
    mut transactions: Vec<&L1HandlerTx>,
) -> (Vec<MessageToStarknet>, Vec<MessageToAppchain>) {
    let message_to_starknet_segment = exec_infos
        .iter()
        .flat_map(|t| t.execute_call_info.iter().chain(t.validate_call_info.iter()).chain(t.fee_transfer_call_info.iter())) // Take into account both validate and execute calls.
        .flat_map(get_messages_recursively)
        .collect();

    let message_to_appchain_segment = exec_infos
        .iter()
        .flat_map(|t| t.execute_call_info.iter())
        .filter(|c| c.entry_point_type == EntryPointType::L1Handler)
        .map(|c| {
            let message_hash =
                compute_l1_message_hash(*c.caller_address, *c.contract_address, &c.calldata[..]);

            // Matching execution to a transaction to extract nonce.
            let matching = transactions
                .iter()
                .enumerate()
                .find(|(_, &t)| {
                    t.message_hash == message_hash
                        && c.contract_address == t.contract_address
                        && c.calldata == t.calldata
                })
                .unwrap_or_else(|| {
                    panic!("No matching transaction found for message hash: {}", message_hash)
                })
                .0;

            // Removing, to have different nonces, even for the same message content.
            let removed = transactions.remove(matching);

            (c, removed)
        })
        .map(|(c, t)| MessageToAppchain {
            from_address: c.caller_address,
            to_address: c.contract_address,
            nonce: t.nonce,
            selector: c.entry_point_selector,
            payload: c.calldata.clone(),
        })
        .collect();

    (message_to_starknet_segment, message_to_appchain_segment)
}

impl ProgramInput {
    // pub fn serialize(&self) -> anyhow::Result<String>{
    //     let message_to_starknet = self
    //         .message_to_starknet_segment
    //         .iter()
    //         .map(MessageToStarknet::serialize)
    //         .collect::<anyhow::Result<Vec<_>>>()?
    //         .into_iter()
    //         .flatten()
    //         .map(|e| format!("{}", e))
    //         .collect::<Vec<_>>()
    //         .join(",");
    //     let message_to_appchain = self
    //         .message_to_appchain_segment
    //         .iter()
    //         .map(|m| m.serialize())
    //         .collect::<anyhow::Result<Vec<_>>>()?
    //         .into_iter()
    //         .flatten()
    //         .map(|e| format!("{}", e))
    //         .collect::<Vec<_>>()
    //         .join(",");

    //     let mut result = String::from('{');
    //     result.push_str(&format!(r#""prev_state_root":{},"#, self.prev_state_root));
    //     result.push_str(&format!(r#""block_number":{},"#, self.block_number));
    //     result.push_str(&format!(r#""block_hash":{},"#, self.block_hash));
    //     result.push_str(&format!(r#""config_hash":{},"#, self.config_hash));

    //     result.push_str(&format!(r#""message_to_starknet_segment":[{}],"#, message_to_starknet));
    //     result.push_str(&format!(r#""message_to_appchain_segment":[{}],"#, message_to_appchain));

    //     result.push_str(&state_updates_to_json_like(&self.state_updates));

    //     result.push_str(&format!("{}", "}"));

    //     Ok(result)
    // }

    pub fn da_as_calldata(&self, world: FieldElement) -> Vec<FieldElement> {
        let updates = self
            .state_updates
            .storage_updates
            .get(&ContractAddress::from(world))
            .unwrap_or(&std::collections::HashMap::new())
            .iter()
            .map(|(k, v)| vec![*k, *v])
            .flatten()
            .collect::<Vec<_>>();

        updates
    }

    pub fn combine(mut self, other: ProgramInput) -> ProgramInput {
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

        // The block number is the one from the last block.
        ProgramInput {
            prev_state_root: self.prev_state_root,
            block_number: other.block_number,
            block_hash: other.block_hash,
            config_hash: self.config_hash,
            message_to_appchain_segment: self.message_to_appchain_segment,
            message_to_starknet_segment: self.message_to_starknet_segment,
            state_updates: self.state_updates,
        }
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
                let num = decimal.to_u64().ok_or_else(|| {
                    serde::ser::Error::custom("Failed to convert BigDecimal to u64")
                })?;
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
                    .next_element::<u64>()?
                    .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                {
                    let to_address = seq
                        .next_element::<u64>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let payload_length = seq.next_element::<usize>()?.unwrap_or_default();
                    let mut payload = Vec::new();
                    for _ in 0..payload_length {
                        if let Some(element) = seq
                            .next_element::<u64>()?
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
                let num = decimal.to_u64().ok_or_else(|| {
                    serde::ser::Error::custom("Failed to convert BigDecimal to u64")
                })?;
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
                    .next_element::<u64>()?
                    .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                {
                    let to_address = seq
                        .next_element::<u64>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let nonce = seq
                        .next_element::<u64>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let selector = seq
                        .next_element::<u64>()?
                        .map(|num| FieldElement::from_str(&num.to_string()).unwrap())
                        .unwrap_or_default();
                    let payload_length = seq.next_element::<usize>()?.unwrap_or_default();
                    let mut payload = Vec::new();
                    for _ in 0..payload_length {
                        if let Some(element) = seq
                            .next_element::<u64>()?
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
fn test_program_input() -> anyhow::Result<()> {
    use std::str::FromStr;

    let input = ProgramInput {
        prev_state_root: FieldElement::from_str("101")?,
        block_number: 102,
        block_hash: FieldElement::from_str("103")?,
        config_hash: FieldElement::from_str("104")?,
        message_to_starknet_segment: vec![MessageToStarknet {
            from_address: ContractAddress::from(FieldElement::from_str("105")?),
            to_address: ContractAddress::from(FieldElement::from_str("106")?),
            payload: vec![FieldElement::from_str("107")?],
        }],
        message_to_appchain_segment: vec![MessageToAppchain {
            from_address: ContractAddress::from(FieldElement::from_str("108")?),
            to_address: ContractAddress::from(FieldElement::from_str("109")?),
            nonce: FieldElement::from_str("110")?,
            selector: FieldElement::from_str("111")?,
            payload: vec![FieldElement::from_str("112")?],
        }],
        state_updates: StateUpdates {
            nonce_updates: std::collections::HashMap::new(),
            storage_updates: vec![(
                ContractAddress::from(FieldElement::from_str("113")?),
                vec![(FieldElement::from_str("114")?, FieldElement::from_str("115")?)]
                    .into_iter()
                    .collect(),
            )]
            .into_iter()
            .collect(),
            contract_updates: std::collections::HashMap::new(),
            declared_classes: std::collections::HashMap::new(),
        },
    };

    // Serialize with the DA.
    let serialized_with_da = input.serialize(FieldElement::from_str("113")?).unwrap();
    println!("Serialized: {}", serialized_with_da);
    pub const EXPECTED_WITH_DA: &str = r#"{
            "prev_state_root": 101,
            "block_number": 102,
            "block_hash": 103,
            "config_hash": 104,
            "message_to_starknet_segment": [105,106,1,107],
            "message_to_appchain_segment": [108,109,110,111,1,112],
            "nonce_updates": {},
            "storage_updates": {"113":{"114":115}},
            "contract_updates": {},
            "declared_classes": {},
            "world_da": [114, 115]
        }"#;

    let expected = EXPECTED_WITH_DA.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    println!("{}", expected);
    assert_eq!(serialized_with_da, expected);

    // Serialize just the DA as calldata.
    let da_calldata = input.da_as_calldata(FieldElement::from_str("113")?);
    assert_eq!(
        da_calldata,
        vec![
            FieldElement::from_str("2")?,
            FieldElement::from_str("114")?,
            FieldElement::from_str("115")?
        ]
    );

    Ok(())
}
