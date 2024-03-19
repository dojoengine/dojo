use anyhow::Ok;
use cairo_proof_parser::parse;
use starknet::core::types::FieldElement;

use super::vec252::VecFelt252;

pub fn parse_proof(proof: String) -> anyhow::Result<Vec<FieldElement>> {
    let parsed = parse(proof)?;

    let config: VecFelt252 = serde_json::from_str(&parsed.config.to_string()).unwrap();
    let public_input: VecFelt252 = serde_json::from_str(&parsed.public_input.to_string()).unwrap();
    let unsent_commitment: VecFelt252 =
        serde_json::from_str(&parsed.unsent_commitment.to_string()).unwrap();
    let witness: VecFelt252 = serde_json::from_str(&parsed.witness.to_string()).unwrap();

    let serialized = config
        .to_vec()
        .into_iter()
        .chain(public_input.to_vec().into_iter())
        .chain(unsent_commitment.to_vec().into_iter())
        .chain(witness.to_vec().into_iter())
        .map(|x| FieldElement::from_dec_str(&x.to_string()))
        .map(Result::unwrap)
        .collect();

    Ok(serialized)
}
