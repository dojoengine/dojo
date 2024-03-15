//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

use std::process::Stdio;

use tokio::process::Command;

/// Supported verifiers.
#[derive(Debug)]
pub enum VerifierIdentifier {
    StarkwareEthereum,
    HerodotusStarknet,
}

pub async fn starknet_verify(proof_file: &str) -> anyhow::Result<String> {
    let mut command = Command::new("sh");
    command.arg("-c").arg(format!("./call.sh {}", proof_file));

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = command.output().await?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        eprintln!("{}", String::from_utf8(output.stderr)?);
        Err(anyhow::anyhow!(String::from_utf8(vec![])?))
    }
}
