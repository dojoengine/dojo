use futures::future::BoxFuture;
use futures::FutureExt;
use tracing::{info, trace};

use super::{prove, ProverIdentifier, ProverInput};

type Proof = String;

async fn combine_proofs(
    first: Proof,
    second: Proof,
    _input: &ProverInput,
) -> anyhow::Result<Proof> {
    // TODO: Insert the real `merge program` here
    let proof: String = first + " & " + &second;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    Ok(proof)
}

// Return type based on: https://rust-lang.githmergeub.io/async-book/07_workarounds/04_recursion.html.
pub fn prove_recursively(
    mut inputs: Vec<ProverInput>,
    prover: ProverIdentifier,
) -> BoxFuture<'static, anyhow::Result<(Proof, ProverInput)>> {
    async move {
        if inputs.len() <= 1 {
            // If only one block to prove, use the passed prover program.
            let input = inputs.pop().unwrap();
            let block_number = input.block_number;
            trace!(target: "saya_core", "Proving block {block_number}.");
            let proof = prove(input.serialize()?, prover).await;
            info!(target: "saya_core", block_number, "Block proven.");

            Ok((proof?, input))
        } else {
            // If more then one prover is remaining to be proven, split them in half and wait for both to be proven.
            let last = inputs.split_off(inputs.len() / 2);

            let (earlier, later) = tokio::try_join!(
                tokio::spawn(async move { prove_recursively(inputs, prover).await }),
                tokio::spawn(async move { prove_recursively(last, prover).await })
            )?;
            let (earlier, later) = (earlier?, later?);

            // Getting the inputs back from the two proofs, to avoid making a copy.
            let input = earlier.1.combine(later.1);

            // Use the `merge program` to make a single proof of combined state diff
            let merged_proofs = combine_proofs(earlier.0, later.0, &input).await?;

            Ok((merged_proofs, input))
        }
    }
    .boxed()
}

// TODO: Migrate this tests to the new inputs.
#[cfg(test)]
mod tests {
    use katana_primitives::FieldElement;

    use crate::prover::{state_diff::ProvedStateDiff, ProverIdentifier};

    use super::prove_recursively;

    // #[tokio::test]
    // async fn test_one() {
    //     let start_instant = std::time::Instant::now();
    //     let inputs = (0..1u64)
    //         .map(|i| ProvedStateDiff {
    //             genesis_state_hash: FieldElement::from(0u64),
    //             prev_state_hash: FieldElement::from(i),
    //             state_updates: Default::default(),
    //         })
    //         .collect::<Vec<_>>();

    //     let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();

    //     assert_eq!(proof.0, "dummy ok".to_string());
    //     assert_eq!(start_instant.elapsed().as_secs(), 1);
    // }

    // #[tokio::test]
    // async fn test_combined() {
    //     let start_instant = std::time::Instant::now();
    //     let inputs = (0..2u64)
    //         .map(|i| ProvedStateDiff {
    //             genesis_state_hash: FieldElement::from(0u64),
    //             prev_state_hash: FieldElement::from(i),
    //             state_updates: Default::default(),
    //         })
    //         .collect::<Vec<_>>();

    //     let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();
    //     assert_eq!(proof, "dummy ok & dummy ok");
    //     assert_eq!(start_instant.elapsed().as_secs(), 2);
    // }

    // #[tokio::test]
    // async fn test_many() {
    //     let start_instant = std::time::Instant::now();
    //     let inputs = (0..8u64)
    //         .map(|i| ProvedStateDiff {
    //             genesis_state_hash: FieldElement::from(0u64),
    //             prev_state_hash: FieldElement::from(i),
    //             state_updates: Default::default(),
    //         })
    //         .collect::<Vec<_>>();

    //     let proof = prove_recursively(inputs, ProverIdentifier::Dummy).await.unwrap();

    //     let expected =
    //         "dummy ok & dummy ok & dummy ok & dummy ok & dummy ok & dummy ok & dummy ok & dummy ok"
    //             .to_string();
    //     assert_eq!(proof, expected);
    //     assert_eq!(start_instant.elapsed().as_secs(), 4);
    // }

}
