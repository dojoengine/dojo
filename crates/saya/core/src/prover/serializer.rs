use anyhow::Ok;
use cairo_proof_parser::parse;
use starknet::core::types::FieldElement;

pub fn parse_proof(proof: &str) -> anyhow::Result<Vec<FieldElement>> {
    Ok(parse(proof)?.into())
}
