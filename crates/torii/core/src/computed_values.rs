use std::collections::HashMap;

use camino::Utf8PathBuf;
use dojo_world::manifest::{abi, ComputedValueEntrypoint, Manifest};
use starknet::core::utils::get_selector_from_name;
use torii_core::types::ComputedValueCall;
use tracing::error;

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
    manifest_json: Option<Utf8PathBuf>,
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
