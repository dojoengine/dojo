use starknet::core::types::FieldElement;

#[derive(Clone, Debug)]
pub struct Query {
    pub address_domain: u32,
    pub partition: FieldElement,
    pub keys: Vec<FieldElement>,
}
