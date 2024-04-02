use std::str::FromStr;

use katana_primitives::contract::ContractAddress;
use starknet::core::types::FieldElement;

/// Based on https://github.com/cartridge-gg/piltover/blob/2be9d46f00c9c71e2217ab74341f77b09f034c81/src/snos_output.cairo#L19-L20
/// With the new state root computed by ten prover.
pub struct ProgramInput {
    prev_state_root: FieldElement,
    block_number: FieldElement,
    block_hash: FieldElement,
    config_hash: FieldElement,
    message_to_starknet_segment: Vec<MessageToStarknet>,
    message_to_appchain_segment: Vec<MessageToAppchain>,
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
struct MessageToStarknet {
    from_address: ContractAddress,
    to_address: ContractAddress,
    payload: Vec<FieldElement>,
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
struct MessageToAppchain {
    from_address: ContractAddress,
    to_address: ContractAddress,
    nonce: FieldElement,
    selector: FieldElement,
    payload: Vec<FieldElement>,
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
