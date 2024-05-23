use super::{MessageToAppchain, MessageToStarknet, ProgramInput};
use anyhow::anyhow;
use katana_primitives::state::StateUpdates;
use katana_primitives::{contract::ContractAddress, FieldElement};
use num_traits::ToPrimitive;

pub fn program_input_from_program_output(
    _output: Vec<FieldElement>,
    _state_updates: StateUpdates,
) -> anyhow::Result<ProgramInput> {
    // println!("{:?}", serde_json::to_string(&output).unwrap());
    // let prev_state_root = output[0].clone();
    // let block_number = serde_json::from_str(&output[2].clone().to_string()).unwrap();
    // let block_hash = output[3].clone();
    // let config_hash = output[4].clone();
    // let message_to_starknet_segment: Vec<MessageToStarknet>;
    // let message_to_appchain_segment: Vec<MessageToAppchain>;
    // let mut decimal = output[6].clone().to_big_decimal(0); // Convert with no decimal places
    // let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
    // match num {
    //     0..=3 => {
    //         message_to_starknet_segment = Default::default(); // TODO: report error here
    //     }
    //     4..=u64::MAX => {
    //         message_to_starknet_segment =
    //             get_message_to_starknet_segment(&output[7..7 + num as usize].to_vec())?
    //     }
    // }
    // let index = 7 + num as usize;
    // decimal = output[index].clone().to_big_decimal(0);
    // let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
    // match num {
    //     0..=4 => {
    //         message_to_appchain_segment = Default::default();
    //     }
    //     5..=u64::MAX => {
    //         message_to_appchain_segment = get_message_to_appchain_segment(
    //             &output[index + 1..index + 1 + num as usize].to_vec(),
    //         )?
    //     }
    // }

    let mut input = ProgramInput { ..Default::default() };

    input.fill_da(FieldElement::default()); // TODO: pass contract address to function
    Ok(input)
}

fn get_message_to_starknet_segment(
    output: &Vec<FieldElement>,
) -> anyhow::Result<Vec<MessageToStarknet>> {
    let mut message_to_starknet_segment: Vec<MessageToStarknet> = vec![];
    let mut index = 0;
    loop {
        if index >= output.len() {
            break;
        }
        let from_address = ContractAddress::from(output[index].clone());
        let to_address = ContractAddress::from(output[index + 1].clone());
        let decimal = output[index + 2].clone().to_big_decimal(0);
        let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
        let payload = output[index + 3..index + 3 + num as usize].to_vec();
        message_to_starknet_segment.push(MessageToStarknet { from_address, to_address, payload });
        index += 3 + num as usize;
    }
    Ok(message_to_starknet_segment)
}

fn get_message_to_appchain_segment(
    output: &Vec<FieldElement>,
) -> anyhow::Result<Vec<MessageToAppchain>> {
    let mut message_to_appchain_segment: Vec<MessageToAppchain> = vec![];
    let mut index = 0;
    loop {
        if index >= output.len() {
            break;
        }
        let from_address = ContractAddress::from(output[index].clone());
        let to_address = ContractAddress::from(output[index + 1].clone());
        let nonce = output[index + 2].clone();
        let selector = output[index + 3].clone();
        let decimal = output[index + 4].clone().to_big_decimal(0);
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
    use crate::prover::{prove_diff, ProverIdentifier};

    use super::*;
    use cairo_proof_parser::output::extract_output;
    use itertools::Itertools;
    use katana_primitives::state::StateUpdates;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_program_input_from_program_output() -> anyhow::Result<()> {
        let input = ProgramInput {
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
            world_da: Some(Vec::new()),
        };
        let serialized_input = serde_json::to_string(&input).unwrap();
        let proof = prove_diff(serialized_input, ProverIdentifier::Stone).await.unwrap();
        let program_output_from_proof = extract_output(&proof).unwrap().program_output;
        let program_input_from_proof = program_input_from_program_output(
            program_output_from_proof,
            input.clone().state_updates,
        )
        .unwrap();
        assert_eq!(input, program_input_from_proof);
        Ok(())
    }
}
