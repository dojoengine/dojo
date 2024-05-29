use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::trace::{CallInfo, EntryPointType};
use katana_primitives::transaction::{L1HandlerTx, TxHash};
use katana_rpc_types::trace::TxExecutionInfo;
use starknet::core::types::FieldElement;

use super::state_diff::state_updates_to_json_like;

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/snos_output.cairo#L19-L20
/// With the new state root computed by the prover.
#[derive(Debug)]
pub struct ProgramInput {
    pub prev_state_root: FieldElement,
    pub block_number: FieldElement,
    pub block_hash: FieldElement,
    pub config_hash: FieldElement,
    pub message_to_starknet_segment: Vec<MessageToStarknet>,
    pub message_to_appchain_segment: Vec<MessageToAppchain>,
    pub state_updates: StateUpdates,
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
    pub fn serialize(&self, world: FieldElement) -> anyhow::Result<String> {
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

        result.push_str(&state_updates_to_json_like(&self.state_updates, world));

        result.push('}');

        Ok(result)
    }

    /// Extracts the storage updates for the given world, and flattens them into a single vector
    /// that represent the serialized DA. The length is not included as the array contains
    /// serialiazed struct with two members: key and value.
    /// TODO: migrate to cainome + simple rust vec for better devX in the future.
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
#[derive(Debug)]
pub struct MessageToStarknet {
    pub from_address: ContractAddress,
    pub to_address: ContractAddress,
    pub payload: Vec<FieldElement>,
}

impl MessageToStarknet {
    pub fn serialize(&self) -> anyhow::Result<Vec<FieldElement>> {
        let mut result = vec![*self.from_address, *self.to_address];
        result.push(FieldElement::from(self.payload.len()));
        result.extend(self.payload.iter().cloned());
        Ok(result)
    }
}

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/messaging/output_process.cairo#L28
#[derive(Debug)]
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
        result.push(FieldElement::from(self.payload.len()));
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

    // Serialize just the DA as calldata. The length is not included, only the array of
    // updates [key, value, key, value...].
    let da_calldata = input.da_as_calldata(FieldElement::from_str("113")?);
    assert_eq!(da_calldata, vec![FieldElement::from_str("114")?, FieldElement::from_str("115")?]);

    Ok(())
}
