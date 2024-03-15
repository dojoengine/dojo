use std::process::Stdio;

use super::{ProverClient, ProverIdentifier};
use anyhow::{bail, Context};
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

pub struct StoneProver(pub String);

#[async_trait]
impl ProverClient for StoneProver {
    fn identifier() -> ProverIdentifier {
        ProverIdentifier::Stone
    }

    async fn setup(&self, source: &str) -> anyhow::Result<()> {
        // podman pull neotheprogramist/verifier:latest
        let mut command = Command::new("podman");
        command.arg("pull").arg(format!("docker.io/{}", source));

        run(command, None).await.context("Failed to pull prover")?;

        // let mut command = Command::new("podman");
        // command.arg("pull").arg(format!("docker.io/{}", verifier));
        // run(command, None).await.context("Failed to pull verifier")?;

        Ok(())
    }

    async fn prove(&self, input: String) -> anyhow::Result<String> {
        let mut command = Command::new("podman");
        command.arg("run").arg("-i").arg("--rm").arg(&self.0);

        run(command, Some(input)).await
    }

    async fn local_verify(proof: String) -> anyhow::Result<()> {
        let mut command = Command::new("podman");
        command.arg("run").arg("-i").arg("--rm").arg("verifier");

        run(command, Some(proof)).await?;

        Ok(())
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
        println!("{}", line);
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
