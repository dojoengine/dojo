use std::process::Stdio;

use anyhow::{bail, Context};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::OnceCell;

use super::{ProverClient, ProverIdentifier};

#[derive(Clone)]
pub struct StoneProver(pub String);

pub async fn prove_stone(input: String) -> anyhow::Result<String> {
    let prover = StoneProver::new().await?;
    prover.prove(input).await
}

pub async fn local_verify(input: String) -> anyhow::Result<String> {
    let prover = StoneProver::new().await?;
    prover.local_verify(input).await?;
    Ok(String::from("ok"))
}

#[async_trait]
impl ProverClient for StoneProver {
    fn identifier() -> ProverIdentifier {
        ProverIdentifier::Stone
    }

    async fn prove(&self, input: String) -> anyhow::Result<String> {
        let mut command = Command::new("podman");
        command.arg("run").arg("-i").arg("--rm").arg(&self.0);

        run(command, Some(input)).await
    }

    async fn local_verify(&self, proof: String) -> anyhow::Result<()> {
        let mut command = Command::new("podman");
        command.arg("run").arg("-i").arg("--rm").arg("verifier");

        run(command, Some(proof)).await?;

        Ok(())
    }
}

impl StoneProver {
    async fn new() -> anyhow::Result<StoneProver> {
        static STONE_PROVER: OnceCell<anyhow::Result<String>> = OnceCell::const_new();

        let prover = "neotheprogramist/state-diff-commitment:latest";

        let result = STONE_PROVER
            .get_or_init(|| async {
                let mut command = Command::new("podman");
                command.arg("pull").arg(format!("docker.io/{}", prover));

                run(command, None).await.context("Failed to pull prover")
            })
            .await;

        if result.is_err() {
            bail!("Failed to pull prover");
        }

        Ok(StoneProver(prover.to_string()))
    }
}

async fn run(mut command: Command, input: Option<String>) -> anyhow::Result<String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());

    let mut child = command.spawn()?;

    if let Some(input) = input {
        let mut stdin = child.stdin.take().context("failed to open stdin")?;

        tokio::spawn(async move {
            stdin.write_all(input.as_bytes()).await.unwrap();
        });
    }

    let stdout = child.stdout.take().context("failed to open stdout")?;
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut out = String::new();
    while let Some(line) = lines.next_line().await? {
        out.push_str(&line);
    }

    let status = child.wait().await?;

    if !status.success() {
        if let Some(mut output) = child.stderr.take() {
            let mut err = Vec::new();
            output.read_to_end(&mut err).await?;

            // Handle error output
            let err = String::from_utf8(err).context("failed to parse stderr")?;
            bail!("Podman error: {}", err)
        };
        bail!("Error without stderr")
    }

    Ok(out)
}
