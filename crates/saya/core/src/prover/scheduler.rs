// Required modules and traits for future and async handling.
use futures::future::BoxFuture;
use futures::FutureExt;
use tracing::level_filters::STATIC_MAX_LEVEL;
use tracing::{info, trace};
// Imports from the parent module.
use super::{prove, stone_image::prove_stone, ProgramInput, ProverIdentifier};
type Proof = String;
use serde_json::Value;

/// Asynchronously combines two proofs into a single proof.
/// It simulates a delay to mimic a time-consuming process and combines the proofs.
async fn combine_proofs(
    first: Vec<String>,
    second: Vec<String>,
    _input: &ProgramInput,
) -> anyhow::Result<Vec<String>> {
    return Ok(first.into_iter().chain(second.into_iter()).collect());
    //extendowanie wektorow
}

/// Simulates the proving process with a placeholder function.
/// Returns a proof string asynchronously.
/// Handles the recursive proving of blocks using asynchronous futures.
/// It returns a BoxFuture to allow for dynamic dispatch of futures, useful in recursive async
/// calls.
pub fn prove_recursively(
    mut inputs: Vec<ProgramInput>,
    prover: ProverIdentifier,
) -> BoxFuture<'static, anyhow::Result<(Vec<String>, ProgramInput)>> {
    async move {
        if inputs.len() == 1 {
            let input = inputs.pop().unwrap();
            let block_number = input.block_number;
            trace!(target: "saya_core", "Proving block {block_number}");
            //let proof = prove(input.serialize()?,prover).await?;
            let proof = prove(input.serialize()?, ProverIdentifier::Stone).await?;
            info!(target: "saya_core", block_number, "Block proven");
            //niech zwraca jednoelementową tablice
            let result = vec![proof];
            Ok((result, input))
        } else {
            let mid = inputs.len() / 2;
            let last = inputs.split_off(mid);

            let (earlier, later) = tokio::try_join!(
                tokio::spawn(async move { prove_recursively(inputs, prover.clone()).await }),
                tokio::spawn(async move { prove_recursively(last, prover).await })
            )?;
            let (earlier, later) = (earlier?, later?);

            let input = earlier.1.combine(later.1);
            let merged_proofs = combine_proofs(earlier.0, later.0, &input).await?;
            Ok((merged_proofs, input))
        }
    }
    .boxed()
}

 #[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use katana_primitives::FieldElement;

    use super::*;

    /// Test case for a single input.
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

        let proof = prove_recursively(inputs.clone(), ProverIdentifier::Stone).await.unwrap().0.pop().unwrap();
        let expected = prove(inputs[0].serialize().unwrap(), ProverIdentifier::Stone).await.unwrap();
        assert_eq!(proof, expected);
    }
}

//     //Test case for combined inputs.
//     #[tokio::test]
//     async fn test_combined() {
//         let inputs = (0..2)
//             .map(|i| ProverInput {
//                 prev_state_root: FieldElement::from(i),
//                 block_number: i,
//                 block_hash: FieldElement::from(i),
//                 config_hash: FieldElement::from(i),
//                 message_to_appchain_segment: Default::default(),
//                 message_to_starknet_segment: Default::default(),
//                 state_updates: Default::default(),
//             })
//             .collect::<Vec<_>>();

//         let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();
//         assert_eq!(proof.0, "dummy 0 & dummy 1");
//     }

//     /// Test case to verify recursive division and combination with many inputs.
//     #[tokio::test]
//     async fn test_many() {
//         let inputs = (0..8)
//             .map(|i| ProverInput {
//                 prev_state_root: FieldElement::from(i),
//                 block_number: i,
//                 block_hash: FieldElement::from(i),
//                 config_hash: FieldElement::from(i),
//                 message_to_appchain_segment: Default::default(),
//                 message_to_starknet_segment: Default::default(),
//                 state_updates: Default::default(),
//             })
//             .collect::<Vec<_>>();

//         let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();
//         let expected =
//             "dummy 0 & dummy 1 & dummy 2 & dummy 3 & dummy 4 & dummy 5 & dummy 6 & dummy 7";
//         assert_eq!(proof.0, expected);
//     }

//     /// Test to measure the time taken for a large number of proofs.
//     #[tokio::test]
//     async fn time_test() {
//         let inputs = (0..512)
//             .map(|i| ProverInput {
//                 prev_state_root: FieldElement::from(i),
//                 block_number: i,
//                 block_hash: FieldElement::from(i),
//                 config_hash: FieldElement::from(i),
//                 message_to_appchain_segment: Default::default(),
//                 message_to_starknet_segment: Default::default(),
//                 state_updates: Default::default(),
//             })
//             .collect::<Vec<_>>();

//         let start = Instant::now();
//         let _proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();
//         let elapsed = start.elapsed();
//         println!("Time elapsed: {:?}", elapsed);

//         let expected_duration = Duration::from_secs(9);
//         let tolerance = 0.1;
//         let lower_bound = expected_duration
//             - Duration::from_secs_f64(tolerance * expected_duration.as_secs_f64());
//         let upper_bound = expected_duration
//             + Duration::from_secs_f64(tolerance * expected_duration.as_secs_f64());
//         assert!(
//             elapsed >= lower_bound && elapsed <= upper_bound,
//             "Test failed: elapsed time is not within the expected range"
//         );
//     }
// }
