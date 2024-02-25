use anyhow::Result;
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::strategy::generate_salt;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::FieldElement;

pub mod auth;
pub mod events;
pub mod execute;
pub mod migration;
pub mod model;
pub mod register;

pub async fn get_contract_address<A: ConnectedAccount + Sync>(
    world: &WorldContract<A>,
    name_or_address: String,
) -> Result<FieldElement> {
    if name_or_address.starts_with("0x") {
        FieldElement::from_hex_be(&name_or_address).map_err(anyhow::Error::from)
    } else {
        let contract_class_hash = world.base().call().await?;
        Ok(starknet::core::utils::get_contract_address(
            generate_salt(&name_or_address),
            contract_class_hash.into(),
            &[],
            world.address,
        ))
    }
}
