use anyhow::bail;
use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::trace::{EntryPointType, TxExecInfo};
use katana_primitives::transaction::L1HandlerTx;
use katana_primitives::utils::transaction::compute_l1_message_hash;
use starknet::core::types::FieldElement;

use super::state_diff::state_updates_to_json_like;

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/snos_output.cairo#L19-L20
/// With the new state root computed by ten prover.
pub struct ProgramInput {
    pub prev_state_root: FieldElement,
    pub block_number: FieldElement,
    pub block_hash: FieldElement,
    pub config_hash: FieldElement,
    pub message_to_starknet_segment: Vec<MessageToStarknet>,
    pub message_to_appchain_segment: Vec<MessageToAppchain>,
    pub state_updates: StateUpdates,
}

pub fn extract_messages(
    exec_infos: &Vec<TxExecInfo>,
    mut transactions: Vec<&L1HandlerTx>,
) -> (Vec<MessageToStarknet>, Vec<MessageToAppchain>) {
    let message_to_starknet_segment = exec_infos
        .iter()
        .map(|t| t.execute_call_info.iter().chain(t.validate_call_info.iter())) // Take into account both validate and execute calls.
        .flatten()
        .map(|c| { // Flatten the recursive call structure.
            let mut to_visit = vec![c];
            let mut all = vec![c];

            while let Some(c) = to_visit.pop() {
                to_visit.extend(c.inner_calls.iter().rev());
                all.extend(c.inner_calls.iter().rev());
            }
            all
        })
        .flatten()
        .map(|c| c.l2_to_l1_messages.iter()) // take all messages
        .flatten()
        .map(|m| MessageToStarknet { // Parse them to the format understood by the prover.
            from_address: m.from_address,
            to_address: ContractAddress::from(m.to_address),
            payload: m.payload.clone(),
        })
        .collect();

    let message_to_appchain_segment = exec_infos
        .iter()
        .map(|t| t.execute_call_info.iter())
        .flatten()
        .filter(|c| c.entry_point_type == EntryPointType::L1Handler)
        .map(|c| {
            let message_hash =
                compute_l1_message_hash(*c.caller_address, *c.contract_address, &c.calldata[..]);

            // Matching execution to an transaction to extract nonce
            let matching = transactions
                .iter()
                .enumerate()
                .find(|(_, &t)| {
                    t.message_hash == message_hash
                        && c.contract_address == t.contract_address
                        && c.calldata == t.calldata
                })
                .expect(&format!(
                    "No matching transaction found for message hash: {}",
                    message_hash
                ))
                .0;

            // Removing, to have different nonces, even for the same massages
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
    pub fn serialize(&self) -> anyhow::Result<String> {
        let message_to_starknet = self
            .message_to_starknet_segment
            .iter()
            .map(MessageToStarknet::serialize)
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join(",");

        let message_to_appchain = self
            .message_to_appchain_segment
            .iter()
            .map(|m| m.serialize())
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join(",");

        let mut result = String::from('{');
        result.push_str(&format!(r#""prev_state_root":{},"#, self.prev_state_root));
        result.push_str(&format!(r#""block_number":{},"#, self.block_number));
        result.push_str(&format!(r#""block_hash":{},"#, self.block_hash));
        result.push_str(&format!(r#""config_hash":{},"#, self.config_hash));

        result.push_str(&format!(r#""message_to_starknet_segment":[{}],"#, message_to_starknet));
        result.push_str(&format!(r#""message_to_appchain_segment":[{}],"#, message_to_appchain));

        result.push_str(&state_updates_to_json_like(&self.state_updates));

        result.push_str(&format!("{}", "}"));

        Ok(result)
    }
}

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/messaging/output_process.cairo#L16
pub struct MessageToStarknet {
    pub from_address: ContractAddress,
    pub to_address: ContractAddress,
    pub payload: Vec<FieldElement>,
}

impl MessageToStarknet {
    pub fn serialize(&self) -> anyhow::Result<Vec<FieldElement>> {
        let mut result = vec![*self.from_address, *self.to_address];
        result.push(FieldElement::try_from(self.payload.len())?);
        result.extend(self.payload.iter().cloned());
        Ok(result)
    }
}

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/messaging/output_process.cairo#L28
pub struct MessageToAppchain {
    pub from_address: ContractAddress,
    pub to_address: ContractAddress,
    pub nonce: FieldElement,
    pub selector: FieldElement,
    pub payload: Vec<FieldElement>,
}

impl MessageToAppchain {
    pub fn serialize(&self) -> anyhow::Result<Vec<FieldElement>> {
        let mut result = vec![*self.from_address, *self.to_address, self.nonce, self.selector];
        result.push(FieldElement::try_from(self.payload.len())?);
        result.extend(self.payload.iter().cloned());
        Ok(result)
    }
}

#[test]
fn test_program_input() -> anyhow::Result<()> {
    use std::str::FromStr;

    let input = ProgramInput {
        prev_state_root: FieldElement::from_str("101")?,
        block_number: FieldElement::from_str("102")?,
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
            storage_updates: std::collections::HashMap::new(),
            contract_updates: std::collections::HashMap::new(),
            declared_classes: std::collections::HashMap::new(),
        },
    };

    let serialized = input.serialize().unwrap();

    println!("Serialized: {}", serialized);

    pub const EXPECTED: &str = r#"{
        "prev_state_root": 101,
        "block_number": 102,
        "block_hash": 103,
        "config_hash": 104,
        "message_to_starknet_segment": [105,106,1,107],
        "message_to_appchain_segment": [108,109,110,111,1,112],
        "nonce_updates": {},
        "storage_updates": {},
        "contract_updates": {},
        "declared_classes": {}
    }"#;

    let expected = EXPECTED.chars().filter(|c| !c.is_whitespace()).collect::<String>();

    println!("{}", expected);

    assert_eq!(serialized, expected);

    Ok(())
}
