use anyhow::Result;
use dojo_world::contracts::WorldContractReader;
use scarb_ui::Ui;
use starknet::core::types::{BlockId, BlockTag, Felt, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;

// use crate::migration::ui::MigrationUi;
use crate::utils::{get_contract_address_from_reader, parse_block_id};

pub async fn call<P: Provider + Sync + Send>(
    ui: &Ui,
    world_reader: WorldContractReader<P>,
    tag_or_address: String,
    entrypoint: String,
    calldata: Vec<Felt>,
    block_id: Option<String>,
) -> Result<()> {
    let contract_address = get_contract_address_from_reader(&world_reader, tag_or_address).await?;
    let block_id = if let Some(block_id) = block_id {
        parse_block_id(block_id)?
    } else {
        BlockId::Tag(BlockTag::Pending)
    };

    let res = world_reader
        .provider()
        .call(
            FunctionCall {
                contract_address,
                entry_point_selector: get_selector_from_name(&entrypoint)?,
                calldata,
            },
            block_id,
        )
        .await;

    match res {
        Ok(output) => {
            println!(
                "[ {} ]",
                output.iter().map(|o| format!("0x{:x}", o)).collect::<Vec<_>>().join(" ")
            );
        }
        Err(e) => {
            //ui.print_hidden_sub(format!("{:?}", e));
            anyhow::bail!(format!(
                "Error calling entrypoint `{}` on address: {:#066x}",
                entrypoint, contract_address
            ));
        }
    }

    Ok(())
}
