use super::{prove, MessageToAppchain, MessageToStarknet, ProgramInput, ProverIdentifier};
use cairo_proof_parser::output::{extract_output, ExtractOutputResult};
use futures::future::BoxFuture;
use futures::FutureExt;
use katana_primitives::{contract::ContractAddress, FieldElement};
use tokio::sync::oneshot;
use tracing::{info, trace};
type Proof = String;
use anyhow::{anyhow, bail};
use katana_primitives::state::StateUpdates;
use num_traits::ToPrimitive;

pub struct Scheduler {
    root_task: BoxFuture<'static, anyhow::Result<(Proof, ProgramInput)>>,
    free_differs: Vec<oneshot::Sender<ProgramInput>>,
}

impl Scheduler {
    pub fn new(capacity: usize, world: FieldElement, prover: ProverIdentifier) -> Self {
        let (senders, receivers): (Vec<_>, Vec<_>) =
            (0..capacity).map(|_| oneshot::channel::<ProgramInput>()).unzip();

        let root_task = prove_recursively(receivers, world, prover);

        Scheduler { root_task, free_differs: senders }
    }

    pub fn is_full(&self) -> bool {
        self.free_differs.is_empty()
    }

    pub fn push_diff(&mut self, input: ProgramInput) -> anyhow::Result<()> {
        if self.is_full() {
            bail!("Scheduler is full");
        }

        let sender = self.free_differs.remove(0);
        if sender.send(input).is_err() {
            bail!("Failed to send input to differ");
        }
        Ok(())
    }

    pub async fn proved(self) -> anyhow::Result<(Proof, ProgramInput)> {
        self.root_task.await
    }

    pub async fn prove_recursively(
        inputs: Vec<ProgramInput>,
        world: FieldElement,
        prover: ProverIdentifier,
    ) -> anyhow::Result<(Proof, ProgramInput)> {
        let mut scheduler = Scheduler::new(inputs.len(), world, prover);
        trace!(target: "saya_core", "pushing inputs to scheduler");
        for input in inputs {
            scheduler.push_diff(input)?;
        }
        info!(target: "saya_core", "inputs pushed to scheduler");
        scheduler.proved().await
    }
}

fn program_input_from_program_output(
    output: Vec<FieldElement>,
    state_updates: StateUpdates,
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

async fn combine_proofs(
    first: Proof,
    second: Proof,
    prover: ProverIdentifier,
    state_updates1: StateUpdates,
    state_updates2: StateUpdates,
) -> anyhow::Result<Proof> {
    let ExtractOutputResult { program_output: program_output1, program_output_hash: _ } =
        extract_output(&first)?;
    let ExtractOutputResult { program_output: program_output2, program_output_hash: _ } =
        extract_output(&second)?;

    let earlier_input = program_input_from_program_output(program_output1, state_updates1).unwrap();
    let later_input = program_input_from_program_output(program_output2, state_updates2).unwrap();

    let _merger_input = ProgramInput::prepare_differ_args(vec![earlier_input, later_input]);

    trace!(target: "saya_core", "Merging proofs");

    let prover =
        if prover == ProverIdentifier::Stone { ProverIdentifier::StoneMerge } else { prover };

    // TODO: remove when proof extraction is working.
    let merger_input = "[2 101 102 103 104 1 1111 22222 1 333 2 44 555 44444 4444 1 66666 7777 1 88888 99999 4 123 456 123 128 6 108 109 110 111 1 112 2 44 555 44444 4444 0 1012 103 1032 1042 1 11112 222222 1 333 2 44 5552 444 44 1 666662 77772 1 888882 999992 4 1232 4562 1232 1282 6 1082 1092 1102 1112 12 1122 2 44 5552 444 44 0]".into();
    let merged_proof = prove(merger_input, prover).await?;

    trace!(target: "saya_core", "Proofs merged");
    Ok(merged_proof)
}

/// Simulates the proving process with a placeholder function.
/// Returns a proof string asynchronously.
/// Handles the recursive proving of blocks using asynchronous futures.
/// It returns a BoxFuture to allow for dynamic dispatch of futures, useful in recursive async
/// calls.
fn prove_recursively(
    mut inputs: Vec<oneshot::Receiver<ProgramInput>>,
    world: FieldElement,
    prover: ProverIdentifier,
) -> BoxFuture<'static, anyhow::Result<(Proof, ProgramInput)>> {
    async move {
        if inputs.len() == 1 {
            let mut input = inputs.pop().unwrap().await.unwrap();
            input.fill_da(world);
            let block_number = input.block_number;
            trace!(target: "saya_core", "Proving block {block_number}");

            let prover_input = ProgramInput::prepare_differ_args(vec![input.clone()]);

            let proof = prove(prover_input, prover).await?;
            info!(target: "saya_core", block_number, "Block proven");
            Ok((proof, input))
        } else {
            let mid = inputs.len() / 2;
            let last = inputs.split_off(mid);

            let provers = (prover.clone(), prover.clone());
            let (earlier_result, later_result) = tokio::try_join!(
                tokio::spawn(async move { prove_recursively(inputs, world, provers.0).await }),
                tokio::spawn(async move { prove_recursively(last, world, provers.1).await }),
            )?;

            let ((earlier_result, earlier_input), (later_result, later_input)) =
                (earlier_result?, later_result?);

            let input = earlier_input.clone().combine(later_input.clone())?;
            let merged_proofs = combine_proofs(
                earlier_result,
                later_result,
                prover,
                earlier_input.state_updates,
                later_input.state_updates,
            )
            .await?;
            info!(target: "saya_core", "Merged proofs");
            Ok((merged_proofs, input))
        }
    }
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let proof = prove(serialized_input, ProverIdentifier::Stone).await.unwrap();
        let program_output_from_proof = extract_output(&proof).unwrap().program_output;
        let program_input_from_proof = program_input_from_program_output(
            program_output_from_proof,
            input.clone().state_updates,
        )
        .unwrap();
        assert_eq!(input, program_input_from_proof);
        Ok(())
    }

    #[tokio::test]
    async fn test_combine_proofs() {
        let input1 = r#"{
            "prev_state_root": "101",
            "block_number": 102,
            "block_hash": "103",
            "config_hash": "104",
            "message_to_starknet_segment": [
                "105",
                "106",
                "1",
                "107"
            ],
            "message_to_appchain_segment": [
                "108",
                "109",
                "110",
                "111",
                "1",
                "112"
            ],
            "nonce_updates": {
                "1111": "22222"
            },
            "storage_updates": {
                "333": {
                    "4444": "555"
                }
            },
            "contract_updates": {
                "66666": "7777"
            },
            "declared_classes": {
                "88888": "99999"
            },
            "world_da": []
        }"#;
        let input2 = r#"{
            "prev_state_root": "201",
            "block_number": 202,
            "block_hash": "203",
            "config_hash": "204",
            "message_to_starknet_segment": [
                "205",
                "206",
                "1",
                "207"
            ],
            "message_to_appchain_segment": [
                "208",
                "209",
                "210",
                "211",
                "1",
                "207"
            ],
            "nonce_updates": {
                "12334": "214354"
            },
            "storage_updates": {
                "333": {
                    "44536346444": "565474555"
                }
            },
            "contract_updates": {
                "4356345": "775468977"
            },
            "declared_classes": {
                "88556753888": "9995764599"
            },
            "world_da": []
        }"#;
        let expected = r#"{
            "prev_state_root": "101",
            "block_number": 202,
            "block_hash": "203",
            "config_hash": "104",
            "message_to_starknet_segment": [
                "105",
                "106",
                "1",
                "107",
                "205",
                "206",
                "1",
                "207"
            ],
            "message_to_appchain_segment": [
                "108",
                "109",
                "110",
                "111",
                "1",
                "112",
                "208",
                "209",
                "210",
                "211",
                "1",
                "207"
            ],
            "nonce_updates": {
                "12334": "214354",
                "1111": "22222"
            },
            "storage_updates": {
                "333": {
                    "44536346444": "565474555",
                    "4444": "555"
                }
            },
            "contract_updates": {
                "4356345": "775468977",
                "66666": "7777"
            },
            "declared_classes": {
                "88556753888": "9995764599",
                "88888": "99999"
            },
            "world_da": [
                "4444",
                "555",
                "44536346444",
                "565474555"
            ]
        }"#;
        let input1: ProgramInput = serde_json::from_str(input1).unwrap();
        let input2: ProgramInput = serde_json::from_str(input2).unwrap();
        let expected: ProgramInput = serde_json::from_str(expected).unwrap();
        let inputs = vec![input1.clone(), input2.clone()];
        let output = Scheduler::prove_recursively(
            inputs,
            FieldElement::from_dec_str("333").unwrap(),
            ProverIdentifier::Stone,
        )
        .await
        .unwrap()
        .1;
        assert_eq!(output, expected);
    }

    #[tokio::test]
    async fn test_4_combine_proofs() -> anyhow::Result<()> {
        let world = FieldElement::from_dec_str("42")?;

        let input_1 = r#"{
            "prev_state_root": "101",
            "block_number": 101,
            "block_hash": "103",
            "config_hash": "104",
            "message_to_starknet_segment": ["105", "106", "1", "1"],
            "message_to_appchain_segment": ["108", "109", "110", "111", "1", "112"],
            "storage_updates": {
                "42": {
                    "2010": "1200",
                    "2012": "1300"
                }
            },
            "nonce_updates": {},
            "contract_updates": {},
            "declared_classes": {}
        }
        "#;

        let input_2 = r#"{
            "prev_state_root": "1011",
            "block_number": 102,
            "block_hash": "1033",
            "config_hash": "104",
            "message_to_starknet_segment": ["135", "136", "1", "1"],
            "message_to_appchain_segment": ["158", "159", "150", "151", "1", "152"],
            "storage_updates": {
                "42": {
                    "2010": "1250",
                    "2032": "1300"
                }
            },
            "nonce_updates": {},
            "contract_updates": {},
            "declared_classes": {}
        }"#;

        let input_3 = r#"{
            "prev_state_root": "10111",
            "block_number": 103,
            "block_hash": "10333",
            "config_hash": "104",
            "message_to_starknet_segment": [],
            "message_to_appchain_segment": [],
            "storage_updates": {
                "42": {
                    "2013": "2"
                }
            },
            "nonce_updates": {},
            "contract_updates": {},
            "declared_classes": {}
        }"#;

        let input_4 = r#"{
            "prev_state_root": "101111",
            "block_number": 104,
            "block_hash": "103333",
            "config_hash": "104",
            "message_to_starknet_segment": ["165", "166", "1", "1"],
            "message_to_appchain_segment": ["168", "169", "160", "161", "1", "162"],
            "storage_updates": {
                "42": {
                    "2010": "1700"
                }
            },
            "nonce_updates": {},
            "contract_updates": {},
            "declared_classes": {}
        }
        "#;

        let expected = r#"{
            "prev_state_root": "101",
            "block_number": 104,
            "block_hash": "103333",
            "config_hash": "104",
            "message_to_starknet_segment": ["105", "106", "1", "1", "135", "136", "1", "1", "165", "166", "1", "1"],
            "message_to_appchain_segment": ["108", "109", "110", "111", "1", "112", "158", "159", "150", "151", "1", "152", "168", "169", "160", "161", "1", "162"],
            "storage_updates": {
                "42": {
                    "2010": "1700",
                    "2012": "1300",
                    "2032": "1300",
                    "2013": "2"
                }
            },
            "nonce_updates": {},
            "contract_updates": {},
            "declared_classes": {},
            "world_da": ["2012", "1300", "2010", "1700", "2032", "1300", "2013", "2"]
        }"#;

        let inputs = vec![input_1, input_2, input_3, input_4]
            .into_iter()
            .map(|input| {
                let mut input = serde_json::from_str::<ProgramInput>(input).unwrap();
                input.fill_da(world);
                input
            })
            .collect_vec();

        let expected = serde_json::from_str::<ProgramInput>(expected).unwrap();

        let (_proof, output) =
            Scheduler::prove_recursively(inputs, world, ProverIdentifier::Stone).await?;
        let mut expected_message_to_appchain = expected.clone().message_to_appchain_segment;
        let mut output_message_to_appchain = output.clone().message_to_appchain_segment;
        expected_message_to_appchain.sort();
        output_message_to_appchain.sort();
        assert_eq!(expected_message_to_appchain, output_message_to_appchain);
        let mut expected_message_to_starknet = expected.clone().message_to_starknet_segment;
        let mut output_message_to_starknet = output.clone().message_to_starknet_segment;
        expected_message_to_starknet.sort();
        output_message_to_starknet.sort();
        assert_eq!(expected_message_to_starknet, output_message_to_starknet);
        let mut expected_world_da = expected.clone().world_da.unwrap();
        let mut output_world_da = output.clone().world_da.unwrap();
        expected_world_da.sort();
        output_world_da.sort();
        assert_eq!(expected_world_da, output_world_da);
        assert_eq!(expected, output);
        Ok(())
    }
}
