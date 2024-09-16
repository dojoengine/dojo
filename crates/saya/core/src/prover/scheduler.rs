use anyhow::{bail, Context};
use cairo_proof_parser::output::{extract_output, ExtractOutputResult};
use futures::future::BoxFuture;
use futures::FutureExt;
use katana_primitives::state::StateUpdates;
use katana_primitives::Felt;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, trace};

use super::{prove_diff, ProgramInput, ProverIdentifier};
use crate::prover::extract::program_input_from_program_output;
use crate::prover::ProveProgram;
use crate::LOG_TARGET;

type Proof = String;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvingState {
    Proving,
    Proved,
    NotPushed,
}
type ProvingStateWithBlock = (u64, ProvingState);

#[allow(missing_debug_implementations)]
pub struct Scheduler {
    root_task: BoxFuture<'static, anyhow::Result<(Proof, ProgramInput)>>,
    free_differs: Vec<oneshot::Sender<ProgramInput>>,
    proving_tasks: Vec<ProvingStateWithBlock>,
    update_channel: mpsc::Receiver<ProvingStateWithBlock>,
    block_range: (u64, u64),
}

impl Scheduler {
    pub fn new(capacity: usize, world: Felt, prover: ProverIdentifier) -> Self {
        let (senders, receivers): (Vec<_>, Vec<_>) =
            (0..capacity).map(|_| oneshot::channel::<ProgramInput>()).unzip();

        let (update_sender, update_channel) = mpsc::channel(capacity * 2);
        let root_task = prove_recursively(receivers, world, prover, update_sender);

        Scheduler {
            root_task,
            free_differs: senders,
            proving_tasks: Vec::with_capacity(capacity),
            update_channel,
            block_range: (u64::MAX, 0),
        }
    }

    pub fn is_full(&self) -> bool {
        self.free_differs.is_empty()
    }

    pub fn push_diff(&mut self, input: ProgramInput) -> anyhow::Result<()> {
        if self.is_full() {
            bail!("Scheduler is full");
        }
        let block_number = input.block_number;

        let sender = self.free_differs.remove(0);

        if sender.send(input).is_err() {
            bail!("Failed to send input to differ");
        }

        self.block_range =
            (self.block_range.0.min(block_number), self.block_range.1.max(block_number));

        Ok(())
    }

    pub async fn proved(self) -> anyhow::Result<(Proof, ProgramInput, (u64, u64))> {
        let (proof, input) = self.root_task.await?;
        Ok((proof, input, self.block_range))
    }

    pub async fn merge(
        inputs: Vec<ProgramInput>,
        world: Felt,
        prover: ProverIdentifier,
    ) -> anyhow::Result<(Proof, ProgramInput)> {
        let mut scheduler = Scheduler::new(inputs.len(), world, prover);
        let number_of_inputs = inputs.len();
        trace!(target: LOG_TARGET, number_of_inputs, "Pushing inputs to scheduler");
        for input in inputs {
            scheduler.push_diff(input)?;
        }
        info!(target: LOG_TARGET, number_of_inputs, "inputs pushed to scheduler");
        let (merged_proof, merged_input, _) = scheduler.proved().await?;
        Ok((merged_proof, merged_input))
    }

    pub async fn query(&mut self, block_number: u64) -> anyhow::Result<ProvingState> {
        while !self.update_channel.is_empty() {
            let (block_number, state) =
                self.update_channel.recv().await.context("Failed to recv")?;

            match state {
                ProvingState::Proved => {
                    if let Some((_, s)) =
                        self.proving_tasks.iter_mut().find(|(n, _)| *n == block_number)
                    {
                        *s = ProvingState::Proved;
                    } else {
                        bail!("Block number {} was not found in proving tasks", block_number);
                    }
                }
                ProvingState::Proving => {
                    self.proving_tasks.push((block_number, ProvingState::Proved));
                }
                _ => {
                    unreachable!("Update should be either Proving or Proved");
                }
            }
        }

        match self.proving_tasks.iter().find(|(n, _)| *n == block_number) {
            Some((_, s)) => Ok(*s),
            None => Ok(ProvingState::NotPushed),
        }
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
    world: Felt,
    number_of_inputs: usize,
) -> anyhow::Result<Proof> {
    let ExtractOutputResult { program_output: program_output1, program_output_hash: _ } =
        extract_output(&first)?;
    let ExtractOutputResult { program_output: program_output2, program_output_hash: _ } =
        extract_output(&second)?;

    let earlier_input =
        program_input_from_program_output(program_output1, state_updates1, world).unwrap();
    let later_input =
        program_input_from_program_output(program_output2, state_updates2, world).unwrap();

    let world = format!("{:x}", world);
    trace!(target: LOG_TARGET, number_of_inputs, world, "Merging proofs");

    let prover_input =
        serde_json::to_string(&CombinedInputs { earlier: earlier_input, later: later_input })?;

    let merged_proof = prove_diff(prover_input, prover, ProveProgram::Merger).await?;

    Ok(merged_proof)
}

/// Handles the recursive proving of blocks using asynchronous futures.
/// Returns a proof string asynchronously.
/// It returns a BoxFuture to allow for dynamic dispatch of futures, useful in recursive async
/// calls.
fn prove_recursively(
    mut inputs: Vec<oneshot::Receiver<ProgramInput>>,
    world: Felt,
    prover: ProverIdentifier,
    update_channel: mpsc::Sender<(u64, ProvingState)>,
) -> BoxFuture<'static, anyhow::Result<(Proof, ProgramInput)>> {
    let handle = tokio::spawn(async move {
        if inputs.len() == 1 {
            let mut input = inputs.pop().unwrap().await.unwrap();
            input.fill_da(world);
            let block_number = input.block_number;
            trace!(target: LOG_TARGET, block_number, "Proving block");
            update_channel.send((block_number, ProvingState::Proving)).await.unwrap();

            let prover_input = serde_json::to_string(&input.clone()).unwrap();
            let proof = prove_diff(prover_input, prover, ProveProgram::Differ).await?;

            info!(target: LOG_TARGET, block_number, "Block proven");
            update_channel.send((block_number, ProvingState::Proved)).await.unwrap();
            Ok((proof, input))
        } else {
            let proof_count = inputs.len();
            let last = inputs.split_off(proof_count / 2);

            let provers = (prover.clone(), prover.clone());

            let second_update_sender = update_channel.clone();
            let (earlier_result, later_result) = tokio::try_join!(
                tokio::spawn(async move {
                    prove_recursively(inputs, world, provers.0, update_channel).await
                }),
                tokio::spawn(async move {
                    prove_recursively(last, world, provers.1, second_update_sender).await
                }),
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
                proof_count,
            )
            .await?;

            let first_proven = earlier_input.block_number;
            info!(target: LOG_TARGET, first_proven, proof_count, "Merged proofs");
            Ok((merged_proofs, input))
        }
    });

    async move { handle.await? }.boxed()
}
