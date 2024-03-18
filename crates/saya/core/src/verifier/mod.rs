//! Verifiers backends.
//!
//! Verifiers are deployed on the verifier layer (chain)
//! where facts and proofs are registered and verified.
//!
//! Verifier implementations are used to provide
//! an interface to query the on-chain verifier, but also
//! submitting facts and proofs.

use std::process::Stdio;

use starknet::{
    accounts::{Account, Call, ExecutionEncoding, SingleOwnerAccount},
    core::{types::FieldElement, utils::get_selector_from_name},
    providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider},
    signers::{LocalWallet, SigningKey},
};
use tokio::process::Command;
use url::Url;

/// Supported verifiers.
#[derive(Debug)]
pub enum VerifierIdentifier {
    StarkwareEthereum,
    HerodotusStarknet,
}

pub async fn starknet_verify_script(proof_file: &str) -> anyhow::Result<String> {
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

pub async fn starknet_verify(serialized_proof: Vec<FieldElement>) -> anyhow::Result<FieldElement> {
    let provider = JsonRpcClient::new(HttpTransport::new(
        Url::parse("https://api.cartridge.gg/x/cgg-verif/katana").unwrap(),
    ));

    let signer = FieldElement::from_hex_be(
        "0x710d3218ae70bf7ec580c620ec81e601a6258ceec2494c4261f916f42667000",
    )
    .expect("invalid signer hex");
    let signer = LocalWallet::from(SigningKey::from_secret_scalar(signer));

    println!("Signer: {:?}", signer);

    let address = FieldElement::from_hex_be(
        "0x76372bcb1d993b9ab059e542a93004962fb70d743b0f10e611df9ffe13c6d64",
    )
    .expect("invalid signer address");

    let chain_id = provider.chain_id().await?;

    let account =
        SingleOwnerAccount::new(provider, signer, address, chain_id, ExecutionEncoding::New);

    let tx = account
        .execute(vec![Call {
            to: FieldElement::from_hex_be(
                "0x1b9c4e973ca9af0456eb6ae4c4576c5134905d8a560e0dfa1b977359e2c40ec",
            )
            .expect("invalid verifier address"),
            selector: get_selector_from_name("verify_and_register_fact").expect("invalid selector"),
            calldata: serialized_proof,
        }])
        .send()
        .await?;

    Ok(tx.transaction_hash)
}
