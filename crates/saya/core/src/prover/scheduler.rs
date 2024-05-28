use anyhow::bail;
use cairo_proof_parser::output::{extract_output, ExtractOutputResult};
use futures::future::BoxFuture;
use futures::FutureExt;
use katana_primitives::state::StateUpdates;
use katana_primitives::FieldElement;
use tokio::sync::oneshot;
use tracing::{info, trace};

use super::{prove_diff, ProgramInput, ProverIdentifier};
use crate::prover::extract::program_input_from_program_output;
use crate::prover::ProveProgram;
use crate::LOG_TARGET;

type Proof = String;

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

    pub async fn merge(
        inputs: Vec<ProgramInput>,
        world: FieldElement,
        prover: ProverIdentifier,
    ) -> anyhow::Result<(Proof, ProgramInput)> {
        let mut scheduler = Scheduler::new(inputs.len(), world, prover);
        trace!(target: LOG_TARGET, "pushing inputs to scheduler");
        for input in inputs {
            scheduler.push_diff(input)?;
        }
        info!(target: LOG_TARGET, "inputs pushed to scheduler");
        scheduler.proved().await
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CombinedInputs {
    earlier: ProgramInput,
    later: ProgramInput,
}

async fn combine_proofs(
    first: Proof,
    second: Proof,
    prover: ProverIdentifier,
    state_updates1: StateUpdates,
    state_updates2: StateUpdates,
    world: FieldElement,
) -> anyhow::Result<Proof> {
    let ExtractOutputResult { program_output: program_output1, program_output_hash: _ } =
        extract_output(&first)?;
    let ExtractOutputResult { program_output: program_output2, program_output_hash: _ } =
        extract_output(&second)?;

    let earlier_input =
        program_input_from_program_output(program_output1, state_updates1, world).unwrap();
    let later_input =
        program_input_from_program_output(program_output2, state_updates2, world).unwrap();

    trace!(target: LOG_TARGET, "Merging proofs");

    let prover_input = if cfg!(feature = "cairo1differ") {
        ProgramInput::prepare_differ_args(vec![earlier_input, later_input]);

        // MOCK: remove when proof extraction is working.
        "[2 101 102 103 104 1 1111 22222 1 333 2 44 555 44444 4444 1 66666 7777 1 88888 99999 4 \
         123 456 123 128 6 108 109 110 111 1 112 2 44 555 44444 4444 0 1012 103 1032 1042 1 11112 \
         222222 1 333 2 44 5552 444 44 1 666662 77772 1 888882 999992 4 1232 4562 1232 1282 6 1082 \
         1092 1102 1112 12 1122 2 44 5552 444 44 0]"
            .into()
    } else {
        serde_json::to_string(&CombinedInputs { earlier: earlier_input, later: later_input })?
    };

    let merged_proof = prove_diff(prover_input, prover, ProveProgram::Merger).await?;

    Ok(merged_proof)
}

/// Handles the recursive proving of blocks using asynchronous futures.
/// Returns a proof string asynchronously.
/// It returns a BoxFuture to allow for dynamic dispatch of futures, useful in recursive async
/// calls.
fn prove_recursively(
    mut inputs: Vec<oneshot::Receiver<ProgramInput>>,
    world: FieldElement,
    prover: ProverIdentifier,
) -> BoxFuture<'static, anyhow::Result<(Proof, ProgramInput)>> {
    let handle = tokio::spawn(async move {
        if inputs.len() == 1 {
            let mut input = inputs.pop().unwrap().await.unwrap();
            input.fill_da(world);
            let block_number = input.block_number;
            trace!(target: LOG_TARGET, "Proving block {block_number}");

            let prover_input = if cfg!(feature = "cairo1differ") {
                ProgramInput::prepare_differ_args(vec![input.clone()])
            } else {
                serde_json::to_string(&input.clone()).unwrap()
            };

            let proof = prove_diff(prover_input, prover, ProveProgram::Differ).await?;
            info!(target: LOG_TARGET, block_number, "Block proven");
            Ok((proof, input))
        } else {
            let proof_count = inputs.len();
            let last = inputs.split_off(proof_count / 2);

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
                world,
            )
            .await?;

            let first_proven = earlier_input.block_number;
            info!(target: LOG_TARGET, first_proven, proof_count, "Merged proofs");
            Ok((merged_proofs, input))
        }
    });

    async move { handle.await? }.boxed()
}
