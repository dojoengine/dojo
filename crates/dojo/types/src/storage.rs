use starknet::core::types::Felt;

#[derive(Clone, Debug)]
pub struct Query {
    pub address_domain: u32,
    pub keys: Vec<Felt>,
}
