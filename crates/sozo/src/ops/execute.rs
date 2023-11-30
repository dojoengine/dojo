use anyhow::{Context, Result};
use dojo_world::metadata::Environment;
use starknet::accounts::{Account, Call};
use starknet::core::types::{BlockId, BlockTag, FieldElement, FunctionCall};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_contract_address, get_selector_from_name,
};
use starknet::macros::selector;
use starknet::providers::Provider;
use starknet_crypto::poseidon_hash_many;

use crate::commands::execute::ExecuteArgs;

pub async fn execute(args: ExecuteArgs, env_metadata: Option<Environment>) -> Result<()> {
    let ExecuteArgs { contract_name, entrypoint, calldata, starknet, account } = args;

    let provider = starknet.provider(env_metadata.as_ref())?;

    let world_address = match env_metadata.as_ref() {
        Some(env) => match env.world_address {
            Some(ref address) => Some(address.clone()),
            None => None,
        },
        None => return Err(anyhow::anyhow!("No World Address found")),
    };

    let contract_address_str = world_address.as_ref().unwrap().as_str();

    let contract_class_hash = provider
        .call(
            FunctionCall {
                contract_address: FieldElement::from_hex_be(contract_address_str).unwrap(),
                entry_point_selector: selector!("base"),
                calldata: Vec::new(),
            },
            &BlockId::Tag(BlockTag::Latest),
        )
        .await?;

    let salt = poseidon_hash_many(
        &contract_name
            .chars()
            .collect::<Vec<_>>()
            .chunks(31)
            .map(|chunk| {
                let s: String = chunk.iter().collect();
                cairo_short_string_to_felt(&s).unwrap()
            })
            .collect::<Vec<_>>(),
    );

    let contract_address = get_contract_address(
        salt,
        contract_class_hash[0],
        &[],
        FieldElement::from_hex_be(contract_address_str).unwrap(),
    );

    let account = account.account(provider, env_metadata.as_ref()).await?;

    let res = account
        .execute(vec![Call {
            calldata,
            to: contract_address,
            selector: get_selector_from_name(&entrypoint).unwrap(),
        }])
        .send()
        .await
        .with_context(|| "Failed to send transaction")?;

    println!("Transaction: {:#x}", res.transaction_hash);

    Ok(())
}
