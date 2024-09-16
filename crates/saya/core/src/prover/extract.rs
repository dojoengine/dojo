use anyhow::anyhow;
use bigdecimal::BigDecimal;
use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::Felt;
use num_traits::ToPrimitive;

use super::{MessageToAppchain, MessageToStarknet, ProgramInput};

pub fn program_input_from_program_output(
    output: Vec<Felt>,
    state_updates: StateUpdates,
    world: Felt,
) -> anyhow::Result<ProgramInput> {
    let prev_state_root = output[0];
    let block_number = serde_json::from_str(&output[2].to_string()).unwrap();
    let block_hash = output[3];
    let config_hash = output[4];
    let mut decimal: BigDecimal = output[6].clone().to_bigint().into(); // Convert with no decimal places
    let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;

    let message_to_starknet_segment = match num {
        0..=3 => Default::default(),
        4..=u64::MAX => get_message_to_starknet_segment(&output[7..7 + num as usize])?,
    };

    let index = 7 + num as usize;
    decimal = output[index].clone().to_bigint().into();
    let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
    let message_to_appchain_segment = match num {
        0..=4 => Default::default(),
        5..=u64::MAX => {
            get_message_to_appchain_segment(&output[index + 1..index + 1 + num as usize])?
        }
    };

    let mut input = ProgramInput {
        prev_state_root,
        block_number,
        block_hash,
        config_hash,
        message_to_starknet_segment,
        message_to_appchain_segment,
        state_updates,
        world_da: None,
    };

    input.fill_da(world);
    Ok(input)
}

fn get_message_to_starknet_segment(output: &[Felt]) -> anyhow::Result<Vec<MessageToStarknet>> {
    let mut message_to_starknet_segment: Vec<MessageToStarknet> = vec![];
    let mut index = 0;
    loop {
        if index >= output.len() {
            break;
        }
        let from_address = ContractAddress::from(output[index]);
        let to_address = ContractAddress::from(output[index + 1]);
        let decimal: BigDecimal = output[index + 2].to_bigint().into();
        let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
        let payload = output[index + 3..index + 3 + num as usize].to_vec();
        message_to_starknet_segment.push(MessageToStarknet { from_address, to_address, payload });
        index += 3 + num as usize;
    }
    Ok(message_to_starknet_segment)
}

fn get_message_to_appchain_segment(output: &[Felt]) -> anyhow::Result<Vec<MessageToAppchain>> {
    let mut message_to_appchain_segment: Vec<MessageToAppchain> = vec![];
    let mut index = 0;
    loop {
        if index >= output.len() {
            break;
        }
        let from_address = ContractAddress::from(output[index]);
        let to_address = ContractAddress::from(output[index + 1]);
        let nonce = output[index + 2];
        let selector = output[index + 3];
        let decimal: BigDecimal = output[index + 4].to_bigint().into();
        let num = decimal.to_u64().ok_or_else(|| anyhow!("Conversion to u64 failed"))?;
        let payload = output[index + 5..index + 5 + num as usize].to_vec();

        message_to_appchain_segment.push(MessageToAppchain {
            from_address,
            to_address,
            nonce,
            selector,
            payload,
        });
        index += 5 + num as usize;
    }
    Ok(message_to_appchain_segment)
}
