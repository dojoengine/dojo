// Required modules and traits for future and async handling.
use futures::future::BoxFuture;
use futures::FutureExt;
use tracing::{info, level_filters::STATIC_MAX_LEVEL, trace};

// Imports from the parent module.
use super::{ProverIdentifier, ProverInput};

type Proof = String;


// Asynchronously combines two proofs into a single proof.
async fn combine_proofs(
    first: Proof,
    second: Proof,
    _input: &ProverInput,
) -> anyhow::Result<Proof> {
    // Placeholder: Combine proofs, the current implementation is simplistic.
    let proof: String = first + " & " + &second;
    // Simulate a delay to mimic a time-consuming process.
    println!("{}: Combining proofs {}",&_input.block_number,proof);
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    Ok(proof)
}
async fn prove(id:String, prover: ProverIdentifier) -> anyhow::Result<String> {
    // Placeholder: Prove the input, the current implementation is simplistic.
    let proof = format!("dummy {}",id).to_string();
    // Simulate a delay to mimic a time-consuming process.
    Ok(proof)

}
// This function handles the recursive proving of blocks using asynchronous futures.
// It returns a BoxFuture to allow for dynamic dispatch of futures, which is useful in recursive async calls.
pub fn prove_recursively(
    mut inputs: Vec<ProverInput>,
    prover: ProverIdentifier,
) -> BoxFuture<'static, anyhow::Result<(Proof, ProverInput)>> {
    async move {
        if inputs.len() == 1 {
            // Handle the base case with only one input.
            let input = inputs.pop().unwrap();
            let block_number = input.block_number;
            trace!(target: "saya_core", "Proving block {block_number}.");
            let proof = prove(input.block_number.to_string(), prover).await;
            info!(target: "saya_core", block_number, "Block proven.");

            Ok((proof?, input))
        } else {
            // Recursive case: split inputs into two halves and process each half recursively.
            let last = inputs.split_off(inputs.len() / 2);
            

            // Parallelize the proving process using tokio::try_join.
            let (earlier, later) = tokio::try_join!(
                tokio::spawn(async move { prove_recursively(inputs, prover).await }),
                tokio::spawn(async move { prove_recursively(last, prover).await })
            )?;
            let (earlier, later) = (earlier?, later?);

            // Combine the results from two halves.
            let input = earlier.1.combine(later.1);

            // Merge the proofs into a single proof.
            let merged_proofs = combine_proofs(earlier.0, later.0, &input).await?;

            Ok((merged_proofs, input))
        }
    }
    .boxed()
}

// Test module to ensure the functionality of recursive proving.
#[cfg(test)]
mod tests {
    use katana_primitives::FieldElement;

    // Imports for testing.
    use crate::prover::{state_diff::ProvedStateDiff, ProverIdentifier,ProverInput};
    use super::prove_recursively;
    use super::combine_proofs;
    use std::time::{Duration, Instant};
    
    // Test the case with one input.
    #[tokio::test]
    async fn test_one() {
        let inputs = (0..1u64)
            .map(|i| ProverInput {
                prev_state_root: FieldElement::from(i),
                block_number: i,
                block_hash: FieldElement::from(i),
                config_hash: FieldElement::from(i),
                message_to_appchain_segment: Default::default(),
                message_to_starknet_segment: Default::default(),
                state_updates: Default::default(),
            })
            .collect::<Vec<_>>();

        let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();

        assert_eq!(proof.0, "dummy 0".to_string());
    }

    // Test the case with two inputs.
    #[tokio::test]
    async fn test_combined() {
        let inputs = (0..2u64)
            .map(|i| ProverInput {
                prev_state_root: FieldElement::from(i),
                block_number: i,
                block_hash: FieldElement::from(i),
                config_hash: FieldElement::from(i),
                message_to_appchain_segment: Default::default(),
                message_to_starknet_segment: Default::default(),
                state_updates: Default::default(),
            })
            .collect::<Vec<_>>();
        let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();
        assert_eq!(proof.0, "dummy 0 & dummy 1");
    }

    // Test the case with many inputs to see if the recursive division and combination works as expected.
    #[tokio::test]
    async fn test_many() {

        let inputs = (0..8u64)
            .map(|i| ProverInput {
                prev_state_root: FieldElement::from(i),
                block_number: i,
                block_hash: FieldElement::from(i),
                config_hash: FieldElement::from(i),
                message_to_appchain_segment: Default::default(),
                message_to_starknet_segment: Default::default(),
                state_updates: Default::default(),
            })
            .collect::<Vec<_>>();

        let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();

        let expected =
            "dummy 0 & dummy 1 & dummy 2 & dummy 3 & dummy 4 & dummy 5 & dummy 6 & dummy 7"
                .to_string();
        assert_eq!(proof.0, expected);
    }
#[tokio::test]
    async fn time_test(){
        let inputs = (0..512u64)
            .map(|i| ProverInput {
                prev_state_root: FieldElement::from(i),
                block_number: i,
                block_hash: FieldElement::from(i),
                config_hash: FieldElement::from(i),
                message_to_appchain_segment: Default::default(),
                message_to_starknet_segment: Default::default(),
                state_updates: Default::default(),
            })
            .collect::<Vec<_>>();
        let start = std::time::Instant::now();
        let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();
        let elapsed = start.elapsed();
        println!("Time elapsed: {:?}", elapsed);
        let _tolerance = 0.1;
        let expected_duration = Duration::from_secs(9);
        let lower_bound = expected_duration - Duration::from_secs_f64(_tolerance * expected_duration.as_secs_f64());
        let upper_bound = expected_duration + Duration::from_secs_f64(_tolerance * expected_duration.as_secs_f64());
        
        assert!(
            elapsed >= lower_bound && elapsed <= upper_bound,
            "Test failed: elapsed time is not within the expected range"
        );
    }

     
}
