use super::{prove, MessageToAppchain, MessageToStarknet, ProgramInput, ProverIdentifier};
use cairo_proof_parser::output::{extract_output, ExtractOutputResult};
use futures::future::BoxFuture;
use futures::FutureExt;
use katana_primitives::{contract::ContractAddress, FieldElement};
use tracing::{info, trace};
type Proof = String;
use anyhow::anyhow;
use num_traits::ToPrimitive;

fn program_input_from_program_output(output: Vec<FieldElement>) -> anyhow::Result<ProgramInput> {
    let prev_state_root = output[0].clone();
    let block_number = serde_json::from_str(&output[2].clone().to_string()).unwrap();
    let block_hash = output[3].clone();
    let config_hash = output[4].clone();
    let mut message_to_starknet = Default::default();
    let mut message_to_appchain = Default::default();
    let mut decimal = output[5].clone().to_big_decimal(0); // Convert with no decimal places
    let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
    let mut index = 5;
    match num {
        0..=3 => {
            message_to_starknet = Default::default();
        }
        4..=u64::MAX => {
            let from_address = ContractAddress::from(output[index + 1].clone());
            let to_address = ContractAddress::from(output[index + 2].clone());
            let payload_size = output[index + 3].clone().to_big_decimal(0).to_u64().unwrap();
            let mut payload = vec![];
            for i in 0..payload_size {
                payload.push(output[index + 4 + (i as usize)].clone());
            }
            index = index + 1 + num as usize;
            message_to_starknet = MessageToStarknet { from_address, to_address, payload };
        }
    }
    decimal = output[index].clone().to_big_decimal(0); // Convert with no decimal places
    let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
    match num {
        0..=4 => {
            message_to_appchain = Default::default();
        }
        5..=u64::MAX => {
            let from_address = ContractAddress::from(output[index + 1].clone());
            let to_address = ContractAddress::from(output[index + 2].clone());
            let nonce = output[index + 3].clone();
            let selector = output[index + 4].clone();
            let payload_size = output[index + 5].clone().to_big_decimal(0).to_u64().unwrap();
            let mut payload = vec![];
            for i in 0..payload_size {
                payload.push(output[index + 6 + i as usize].clone());
            }
            message_to_appchain =
                MessageToAppchain { from_address, to_address, nonce, selector, payload };
        }
    }
    Ok(ProgramInput {
        prev_state_root,
        block_number,
        block_hash,
        config_hash,
        message_to_starknet_segment: vec![message_to_starknet],
        message_to_appchain_segment: vec![message_to_appchain],
        state_updates: Default::default(),
    })
}

async fn input_to_json(result: Vec<ProgramInput>) -> anyhow::Result<String> {
    let input1 = serde_json::to_string(
        &result.get(0).ok_or_else(|| anyhow::anyhow!("Index out of bounds")).unwrap(),
    )
    .unwrap();
    let input2 = serde_json::to_string(
        &result.get(1).ok_or_else(|| anyhow::anyhow!("Index out of bounds")).unwrap(),
    )
    .unwrap();
    Ok(format!("{{\"1\":{},\"2\":{}}}", input1, input2))
}
async fn combine_proofs(
    first: Proof,
    second: Proof,
    _input: &ProgramInput,
) -> anyhow::Result<Proof> {
    let ExtractOutputResult { program_output: program_output1, program_output_hash: _ } =
        extract_output(&first)?;
    let ExtractOutputResult { program_output: program_output2, program_output_hash: _ } =
        extract_output(&second)?;

    let program_input1 = program_input_from_program_output(program_output1).unwrap();
    let program_input2 = program_input_from_program_output(program_output2).unwrap();
    //combine two inputs to 1 input.json
    let inputs = vec![program_input1, program_input2];
    Ok(prove(
        input_to_json(inputs).await?,
        ProverIdentifier::Stone,
        "neotheprogramist/merger:latest",
    )
    .await
    .unwrap()
    .to_string())
}

/// Simulates the proving process with a placeholder function.
/// Returns a proof string asynchronously.
/// Handles the recursive proving of blocks using asynchronous futures.
/// It returns a BoxFuture to allow for dynamic dispatch of futures, useful in recursive async
/// calls.
pub fn prove_recursively(
    mut inputs: Vec<ProgramInput>,
    prover: ProverIdentifier,
) -> BoxFuture<'static, anyhow::Result<(Proof, ProgramInput)>> {
    async move {
        if inputs.len() == 1 {
            let input = inputs.pop().unwrap();
            let block_number = input.block_number;
            trace!(target: "saya_core", "Proving block {block_number}");
            let proof =
                prove(serde_json::to_string(&input)?, prover, "piniom/state-diff-commitment")
                    .await?;
            info!(target: "saya_core", block_number, "Block proven");
            Ok((proof, input))
        } else {
            let mid = inputs.len() / 2;
            let last = inputs.split_off(mid);

            let (earlier_result, later_result) = tokio::try_join!(
                tokio::spawn(async move { prove_recursively(inputs, prover.clone()).await }),
                tokio::spawn(async move { prove_recursively(last, prover).await }),
            )?;

            let (earlier_result, later_result) = (earlier_result?, later_result?);

            let input = earlier_result.1.combine(later_result.1);
            let merged_proofs = combine_proofs(earlier_result.0, later_result.0, &input).await?;
            Ok((merged_proofs, input))
        }
    }
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prover::prove_stone;
    use katana_primitives::state::StateUpdates;
    use katana_primitives::FieldElement;
    use std::str::FromStr;
    #[tokio::test]
    async fn test_one() {
        let inputs = (0..1)
            .map(|i| ProgramInput {
                prev_state_root: FieldElement::from(i),
                block_number: i,
                block_hash: FieldElement::from(i),
                config_hash: FieldElement::from(i),
                message_to_appchain_segment: Default::default(),
                message_to_starknet_segment: Default::default(),
                state_updates: Default::default(),
            })
            .collect::<Vec<_>>();

        let proof = prove_recursively(inputs.clone(), ProverIdentifier::Stone).await.unwrap().0;
        let expected =
            prove_stone(serde_json::to_string(&inputs[0]).unwrap(), "piniom/state-diff-commitment")
                .await
                .unwrap();
        assert_eq!(proof, expected);
    }

    #[tokio::test]
    async fn test_program_input_from_program_output() {
        let input = ProgramInput {
            prev_state_root: FieldElement::from_str("101").unwrap(),
            block_number: 102,
            block_hash: FieldElement::from_str("103").unwrap(),
            config_hash: FieldElement::from_str("104").unwrap(),
            message_to_starknet_segment: vec![MessageToStarknet {
                from_address: ContractAddress::from(FieldElement::from_str("105").unwrap()),
                to_address: ContractAddress::from(FieldElement::from_str("106").unwrap()),
                payload: vec![FieldElement::from_str("107").unwrap()],
            }],
            message_to_appchain_segment: vec![MessageToAppchain {
                from_address: ContractAddress::from(FieldElement::from_str("108").unwrap()),
                to_address: ContractAddress::from(FieldElement::from_str("109").unwrap()),
                nonce: FieldElement::from_str("110").unwrap(),
                selector: FieldElement::from_str("111").unwrap(),
                payload: vec![FieldElement::from_str("112").unwrap()],
            }],
            state_updates: StateUpdates {
                nonce_updates: std::collections::HashMap::new(),
                storage_updates: std::collections::HashMap::new(),
                contract_updates: std::collections::HashMap::new(),
                declared_classes: std::collections::HashMap::new(),
            },
        };
        let proof = prove(
            serde_json::to_string(&input).unwrap(),
            ProverIdentifier::Stone,
            "piniom/state-diff-commitment",
        )
        .await
        .unwrap();
        let program_output = extract_output(&proof).unwrap().program_output;
        let program_input = program_input_from_program_output(program_output).unwrap();
        assert_eq!(input, program_input);
    }
    #[tokio::test]
    async fn test_combine_proofs() {
        let input1 = ProgramInput {
            prev_state_root: FieldElement::from_str("101").unwrap(),
            block_number: 102,
            block_hash: FieldElement::from_str("103").unwrap(),
            config_hash: FieldElement::from_str("104").unwrap(),
            message_to_starknet_segment: vec![MessageToStarknet {
                from_address: ContractAddress::from(FieldElement::from_str("105").unwrap()),
                to_address: ContractAddress::from(FieldElement::from_str("106").unwrap()),
                payload: vec![FieldElement::from_str("107").unwrap()],
            }],
            message_to_appchain_segment: vec![MessageToAppchain {
                from_address: ContractAddress::from(FieldElement::from_str("108").unwrap()),
                to_address: ContractAddress::from(FieldElement::from_str("109").unwrap()),
                nonce: FieldElement::from_str("110").unwrap(),
                selector: FieldElement::from_str("111").unwrap(),
                payload: vec![FieldElement::from_str("112").unwrap()],
            }],
            state_updates: StateUpdates {
                nonce_updates: std::collections::HashMap::new(),
                storage_updates: std::collections::HashMap::new(),
                contract_updates: std::collections::HashMap::new(),
                declared_classes: std::collections::HashMap::new(),
            },
        };
        let input2 = ProgramInput {
            prev_state_root: FieldElement::from_str("201").unwrap(),
            block_number: 202,
            block_hash: FieldElement::from_str("203").unwrap(),
            config_hash: FieldElement::from_str("204").unwrap(),
            message_to_starknet_segment: vec![MessageToStarknet {
                from_address: ContractAddress::from(FieldElement::from_str("205").unwrap()),
                to_address: ContractAddress::from(FieldElement::from_str("206").unwrap()),
                payload: vec![FieldElement::from_str("207").unwrap()],
            }],
            message_to_appchain_segment: vec![MessageToAppchain {
                from_address: ContractAddress::from(FieldElement::from_str("208").unwrap()),
                to_address: ContractAddress::from(FieldElement::from_str("209").unwrap()),
                nonce: FieldElement::from_str("210").unwrap(),
                selector: FieldElement::from_str("211").unwrap(),
                payload: vec![FieldElement::from_str("207").unwrap()],
            }],
            state_updates: StateUpdates {
                nonce_updates: std::collections::HashMap::new(),
                storage_updates: std::collections::HashMap::new(),
                contract_updates: std::collections::HashMap::new(),
                declared_classes: std::collections::HashMap::new(),
            },
        };
        let inputs = vec![input1.clone(), input2];
        let proof = prove_recursively(inputs, ProverIdentifier::Stone).await.unwrap().0;
        let extracted_output = extract_output(&proof).unwrap().program_output;
        let left_proof = prove(
            serde_json::to_string(&input1).unwrap(),
            ProverIdentifier::Stone,
            "piniom/state-diff-commitment",
        )
        .await
        .unwrap();
        let left_extracted_output = extract_output(&left_proof).unwrap().program_output;
        let mut new_vector = left_extracted_output[..1].to_vec(); // Takes elements before index 1
        new_vector.extend_from_slice(&left_extracted_output[2..]);
        assert_eq!(extracted_output, new_vector);
    }
}
