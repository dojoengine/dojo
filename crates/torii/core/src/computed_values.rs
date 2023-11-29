use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use dojo_world::manifest::{abi, ComputedValueEntrypoint, Manifest};
use sqlx::{Pool, Sqlite};
use starknet::core::types::{BlockId, BlockTag, FunctionCall};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;
use tracing::error;

use crate::types::ComputedValueCall;

pub fn function_input_output_from_abi(
    computed_val_fn: &ComputedValueEntrypoint,
    abi: &Option<abi::Contract>,
) -> Result<(Vec<abi::Input>, Vec<abi::Output>), String> {
    match abi {
        Some(abi) => abi
            .clone()
            .into_iter()
            .find_map(|i| {
                if let abi::Item::Function(fn_item) = i {
                    if fn_item.name != computed_val_fn.entrypoint {
                        return None;
                    }
                    return Some(Ok((fn_item.inputs, fn_item.outputs)));
                }
                None
            })
            .unwrap(),
        None => Err("Couldn't find the function in the ABI.".into()),
    }
}

pub fn computed_value_entrypoints(
    manifest_json: Option<PathBuf>,
) -> HashMap<String, Vec<ComputedValueCall>> {
    let mut computed_values: HashMap<String, Vec<ComputedValueCall>> = HashMap::new();
    if let Some(manifest) = manifest_json {
        match Manifest::load_from_path(manifest) {
            Ok(manifest) => {
                manifest.contracts.iter().for_each(|contract| {
                    contract.computed.iter().for_each(|computed_val_fn| {
                        let model_name = match computed_val_fn.model.clone() {
                            Some(m_name) => m_name,
                            None => "".into(),
                        };

                        match function_input_output_from_abi(computed_val_fn, &contract.abi) {
                            Ok((input, output)) => {
                                let contract_entrypoint = ComputedValueCall {
                                    contract_name: computed_val_fn.contract.to_string(),
                                    contract_address: contract.address.expect(&format!(
                                        "Contract {} doesn't have an address.",
                                        computed_val_fn.contract.to_string()
                                    )),
                                    entry_point: computed_val_fn.entrypoint.to_string(),
                                    entry_point_selector: get_selector_from_name(
                                        &computed_val_fn.entrypoint.to_string(),
                                    )
                                    .unwrap(),
                                    input,
                                    output,
                                };
                                match computed_values.get_mut(&model_name) {
                                    Some(model_computed_values) => {
                                        model_computed_values.push(contract_entrypoint);
                                    }
                                    None => {
                                        computed_values
                                            .insert(model_name, vec![contract_entrypoint]);
                                    }
                                };
                            }
                            Err(err) => {
                                eprintln!("{err}");
                            }
                        }
                    })
                });
            }
            Err(err) => {
                error!("Manifest error: \n{:?}", err);
            }
        }
        // model
    };
    computed_values
}

pub async fn call_computed_value<P: Provider + Sync>(
    contract_name: &str,
    entry_point: &str,
    calldata: Vec<FieldElement>,
    pool: Pool<Sqlite>,
    provider: &P,
) -> anyhow::Result<Vec<String>> {
    let (contract_address, _input, _output): (String, String, String) =
        sqlx::query_as("SELECT contract_address, input, output FROM computed_values WHERE id = ?")
            .bind(format!("{}::{}", contract_name, entry_point))
            .fetch_one(&pool)
            .await?;

    let entry_point_selector = get_selector_from_name(entry_point).expect("invalid selector name");
    let values = provider
        .call(
            FunctionCall {
                calldata,
                contract_address: FieldElement::from_str(&contract_address).unwrap(),
                entry_point_selector,
            },
            BlockId::Tag(BlockTag::Latest),
        )
        .await?;

    Ok(values.iter().map(|v| format!("{v:x}")).collect())
}
