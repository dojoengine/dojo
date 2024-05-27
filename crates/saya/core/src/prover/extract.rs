use anyhow::anyhow;
use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::FieldElement;
use num_traits::ToPrimitive;

use super::{MessageToAppchain, MessageToStarknet, ProgramInput};

pub fn program_input_from_program_output(
    output: Vec<FieldElement>,
    state_updates: StateUpdates,
    world: FieldElement,
) -> anyhow::Result<ProgramInput> {
    let prev_state_root = output[0];
    let block_number = serde_json::from_str(&output[2].to_string()).unwrap();
    let block_hash = output[3];
    let config_hash = output[4];
    let mut decimal = output[6].clone().to_big_decimal(0); // Convert with no decimal places
    let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;

    let message_to_starknet_segment = match num {
        0..=3 => Default::default(),
        4..=u64::MAX => get_message_to_starknet_segment(&output[7..7 + num as usize])?,
    };

    let index = 7 + num as usize;
    decimal = output[index].clone().to_big_decimal(0);
    let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
    let message_to_appchain_segment = match num {
        0..=4 => Default::default(),
        5..=u64::MAX => {
            get_message_to_appchain_segment(&output[index + 1..index + 1 + num as usize])?
        }
    };

    let mut input = ProgramInput {
        prev_state_root,
        block_number,
        block_hash,
        config_hash,
        message_to_starknet_segment,
        message_to_appchain_segment,
        state_updates,
        world_da: None,
    };

    input.fill_da(world);
    Ok(input)
}

fn get_message_to_starknet_segment(
    output: &[FieldElement],
) -> anyhow::Result<Vec<MessageToStarknet>> {
    let mut message_to_starknet_segment: Vec<MessageToStarknet> = vec![];
    let mut index = 0;
    loop {
        if index >= output.len() {
            break;
        }
        let from_address = ContractAddress::from(output[index]);
        let to_address = ContractAddress::from(output[index + 1]);
        let decimal = output[index + 2].to_big_decimal(0);
        let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
        let payload = output[index + 3..index + 3 + num as usize].to_vec();
        message_to_starknet_segment.push(MessageToStarknet { from_address, to_address, payload });
        index += 3 + num as usize;
    }
    Ok(message_to_starknet_segment)
}

fn get_message_to_appchain_segment(
    output: &[FieldElement],
) -> anyhow::Result<Vec<MessageToAppchain>> {
    let mut message_to_appchain_segment: Vec<MessageToAppchain> = vec![];
    let mut index = 0;
    loop {
        if index >= output.len() {
            break;
        }
        let from_address = ContractAddress::from(output[index]);
        let to_address = ContractAddress::from(output[index + 1]);
        let nonce = output[index + 2];
        let selector = output[index + 3];
        let decimal = output[index + 4].to_big_decimal(0);
        let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
        let payload = output[index + 5..index + 5 + num as usize].to_vec();

        message_to_appchain_segment.push(MessageToAppchain {
            from_address,
            to_address,
            nonce,
            selector,
            payload,
        });
        index += 5 + num as usize;
    }
    Ok(message_to_appchain_segment)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use cairo_proof_parser::output::extract_output;
    use starknet_crypto::FieldElement;

    use super::*;
    use crate::prover::{prove_diff, ProverIdentifier};

    #[tokio::test]
    async fn test_program_input_from_program_output() -> anyhow::Result<()> {
        let mut input = ProgramInput {
            prev_state_root: FieldElement::from_str("101").unwrap(),
            block_number: 102,
            block_hash: FieldElement::from_str("103").unwrap(),
            config_hash: FieldElement::from_str("104").unwrap(),
            message_to_starknet_segment: vec![
                MessageToStarknet {
                    from_address: ContractAddress::from(FieldElement::from_str("105").unwrap()),
                    to_address: ContractAddress::from(FieldElement::from_str("106").unwrap()),
                    payload: vec![FieldElement::from_str("107").unwrap()],
                },
                MessageToStarknet {
                    from_address: ContractAddress::from(FieldElement::from_str("105").unwrap()),
                    to_address: ContractAddress::from(FieldElement::from_str("106").unwrap()),
                    payload: vec![FieldElement::from_str("107").unwrap()],
                },
            ],
            message_to_appchain_segment: vec![
                MessageToAppchain {
                    from_address: ContractAddress::from(FieldElement::from_str("108").unwrap()),
                    to_address: ContractAddress::from(FieldElement::from_str("109").unwrap()),
                    nonce: FieldElement::from_str("110").unwrap(),
                    selector: FieldElement::from_str("111").unwrap(),
                    payload: vec![FieldElement::from_str("112").unwrap()],
                },
                MessageToAppchain {
                    from_address: ContractAddress::from(FieldElement::from_str("108").unwrap()),
                    to_address: ContractAddress::from(FieldElement::from_str("109").unwrap()),
                    nonce: FieldElement::from_str("110").unwrap(),
                    selector: FieldElement::from_str("111").unwrap(),
                    payload: vec![FieldElement::from_str("112").unwrap()],
                },
            ],
            state_updates: StateUpdates {
                nonce_updates: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        ContractAddress::from(FieldElement::from_str("1111").unwrap()),
                        FieldElement::from_str("22222").unwrap(),
                    );
                    map
                },
                storage_updates: vec![(
                    ContractAddress::from(FieldElement::from_str("333")?),
                    vec![(FieldElement::from_str("4444")?, FieldElement::from_str("555")?)]
                        .into_iter()
                        .collect(),
                )]
                .into_iter()
                .collect(),
                contract_updates: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        ContractAddress::from(FieldElement::from_str("66666").unwrap()),
                        FieldElement::from_str("7777").unwrap(),
                    );
                    map
                },
                declared_classes: {
                    let mut map = std::collections::HashMap::new();
                    map.insert(
                        FieldElement::from_str("88888").unwrap(),
                        FieldElement::from_str("99999").unwrap(),
                    );
                    map
                },
            },
            world_da: None,
        };

        input.fill_da(333u64.into());

        let serialized_input = serde_json::to_string(&input).unwrap();
        let proof = prove_diff(serialized_input, ProverIdentifier::Stone).await.unwrap();
        let program_output_from_proof = extract_output(&proof).unwrap().program_output;
        let program_input_from_proof = program_input_from_program_output(
            program_output_from_proof,
            input.clone().state_updates,
            333u64.into(),
        )
        .unwrap();
        assert_eq!(input, program_input_from_proof);
        Ok(())
    }
}
