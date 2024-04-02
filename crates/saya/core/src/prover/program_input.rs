use katana_primitives::contract::ContractAddress;
use katana_primitives::trace::{EntryPointType, TxExecInfo};
use starknet::core::types::FieldElement;

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/snos_output.cairo#L19-L20
/// With the new state root computed by ten prover.
pub struct ProgramInput {
    pub prev_state_root: FieldElement,
    pub block_number: FieldElement,
    pub block_hash: FieldElement,
    pub config_hash: FieldElement,
    pub message_to_starknet_segment: Vec<MessageToStarknet>,
    pub message_to_appchain_segment: Vec<MessageToAppchain>,
}

pub fn extract_messages(
    exec_infos: &Vec<TxExecInfo>,
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
        .map(|c| MessageToAppchain {
            from_address: c.caller_address,
            to_address: c.contract_address,
            nonce: FieldElement::from(0u64), // TODO: extract nonce
            selector: c.entry_point_selector,
            payload: c.calldata.clone(),
        })
        .collect();

    (message_to_starknet_segment, message_to_appchain_segment)
}

impl ProgramInput {
    pub fn serialize(&self) -> anyhow::Result<Vec<FieldElement>> {
        let mut result =
            vec![self.prev_state_root, self.block_number, self.block_hash, self.config_hash];

        let message_to_starknet = self
            .message_to_starknet_segment
            .iter()
            .map(|m| m.serialize())
            .collect::<anyhow::Result<Vec<_>>>()?;

        result.push(FieldElement::try_from(self.message_to_starknet_segment.len())?);
        result.extend(message_to_starknet.into_iter().flatten());

        let message_to_appchain = self
            .message_to_appchain_segment
            .iter()
            .map(|m| m.serialize())
            .collect::<anyhow::Result<Vec<_>>>()?;
        result.push(FieldElement::try_from(self.message_to_appchain_segment.len())?);
        result.extend(message_to_appchain.into_iter().flatten());

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
    };

    let serialized = input.serialize().unwrap();
    assert_eq!(serialized.len(), 16);

    let expected_serialized = vec![
        FieldElement::from_str("101")?,
        FieldElement::from_str("102")?,
        FieldElement::from_str("103")?,
        FieldElement::from_str("104")?,
        FieldElement::from_str("1")?,
        FieldElement::from_str("105")?,
        FieldElement::from_str("106")?,
        FieldElement::from_str("1")?,
        FieldElement::from_str("107")?,
        FieldElement::from_str("1")?,
        FieldElement::from_str("108")?,
        FieldElement::from_str("109")?,
        FieldElement::from_str("110")?,
        FieldElement::from_str("111")?,
        FieldElement::from_str("1")?,
        FieldElement::from_str("112")?,
    ];

    assert_eq!(serialized, expected_serialized);

    Ok(())
}
