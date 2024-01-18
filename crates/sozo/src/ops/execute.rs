use anyhow::{Context, Result};
use dojo_world::metadata::Environment;
use dojo_world::migration::strategy::generate_salt;
use dojo_world::utils::TransactionWaiter;
use starknet::accounts::{Account, Call};
use starknet::core::types::{BlockId, BlockTag, FieldElement, FunctionCall};
use starknet::core::utils::{get_contract_address, get_selector_from_name};
use starknet::macros::selector;
use starknet::providers::Provider;

use crate::commands::execute::ExecuteArgs;

pub async fn execute(args: ExecuteArgs, env_metadata: Option<Environment>) -> Result<()> {
    let ExecuteArgs { contract, entrypoint, calldata, starknet, account, transaction } = args;

    let provider = starknet.provider(env_metadata.as_ref())?;

    let contract_address = if contract.starts_with("0x") {
        FieldElement::from_hex_be(&contract)?
    } else {
        let world_address = env_metadata
            .as_ref()
            .and_then(|env| env.world_address.as_ref())
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No World Address found"))?;

        let contract_class_hash = provider
            .call(
                FunctionCall {
                    contract_address: FieldElement::from_hex_be(&world_address).unwrap(),
                    entry_point_selector: selector!("base"),
                    calldata: [].to_vec(),
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await?;

        get_contract_address(
            generate_salt(&contract),
            contract_class_hash[0],
            &[],
            FieldElement::from_hex_be(&world_address).unwrap(),
        )
    };

    let account = account.account(&provider, env_metadata.as_ref()).await?;

    let res = account
        .execute(vec![Call {
            calldata,
            to: contract_address,
            selector: get_selector_from_name(&entrypoint)?,
        }])
        .send()
        .await
        .with_context(|| "Failed to send transaction")?;

    if transaction.wait {
        let receipt = TransactionWaiter::new(res.transaction_hash, &provider).await?;
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else {
        println!("Transaction hash: {:#x}", res.transaction_hash);
    }

    Ok(())
}
