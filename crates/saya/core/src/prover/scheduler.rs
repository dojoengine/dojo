use futures::future::BoxFuture;
use futures::FutureExt;

use super::state_diff::ProvedStateDiff;
use super::{prove, ProverIdentifier};

type Proof = String;

async fn combine_proofs(first: Proof, second: Proof) -> anyhow::Result<Proof> {
    // TODO: Insert the real `merge program` here
    let proof = first + &second;

    Ok(proof)
}

// Return type based on: https://rust-lang.github.io/async-book/07_workarounds/04_recursion.html.
pub fn prove_recursively(
    mut inputs: Vec<ProvedStateDiff>,
    prover: ProverIdentifier,
) -> BoxFuture<'static, anyhow::Result<Proof>> {
    async move {
        if inputs.len() <= 1 {
            prove(inputs[0].serialize(), prover).await
        } else {
            let last = inputs.split_off(inputs.len() / 2);

            let proofs = tokio::try_join!(
                tokio::spawn(async move { prove_recursively(inputs, prover).await }),
                tokio::spawn(async move { prove_recursively(last, prover).await })
            )?;

            combine_proofs(proofs.0?, proofs.1?).await
        }
    }
    .boxed()
}

#[cfg(test)]
mod tests {
    async fn test_mine() {
        for i in 0..1000000000 {
            let _ = i;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_parallelism() {
        let now = tokio::time::Instant::now();

        let a = tokio::spawn(async {
            test_mine().await;
        });

        let b = tokio::spawn(async {
            test_mine().await;
        });

        tokio::try_join!(a, b).unwrap();

        print!("{}", now.elapsed().as_secs())
    }
}
