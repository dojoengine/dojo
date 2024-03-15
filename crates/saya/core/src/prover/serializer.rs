use anyhow::Ok;
use cairo_felt::Felt252;
use cairo_proof_parser::parse;
use itertools::{chain, Itertools};

use super::vec252::VecFelt252;

pub fn parse_proof(proof: String) -> anyhow::Result<Vec<Felt252>> {
    let parsed = parse(proof)?;

    let config: VecFelt252 = serde_json::from_str(&parsed.config.to_string()).unwrap();
    let public_input: VecFelt252 = serde_json::from_str(&parsed.public_input.to_string()).unwrap();
    let unsent_commitment: VecFelt252 =
        serde_json::from_str(&parsed.unsent_commitment.to_string()).unwrap();
    let witness: VecFelt252 = serde_json::from_str(&parsed.witness.to_string()).unwrap();

    let serialized = chain!(
        config.to_vec(),
        public_input.to_vec(),
        unsent_commitment.to_vec(),
        witness.to_vec()
    )
    .collect_vec();

    Ok(serialized)
}
