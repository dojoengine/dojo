use std::env::{self, current_dir};

use camino::Utf8PathBuf;
use clap::Args;

use serde_json::{Value};
use std::fs::{File, read_dir};
use std::io::prelude::*;

#[derive(Args)]
pub struct BindArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,
}

#[derive(Debug, serde::Deserialize)]
struct AbiFile {
    abi: Vec<Value>,
}

pub fn run(args: BindArgs) -> anyhow::Result<()>  {
    let input_dir = match args.path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                Utf8PathBuf::from_path_buf(current_path).unwrap()
            }
        }
        None => Utf8PathBuf::from_path_buf(current_dir().unwrap()).unwrap(),
    };

    for entry in read_dir(input_dir).expect("Unable to read ABI directory") {
        let entry = entry.expect("Unable to read entry");
        let abi_path = entry.path();



        if abi_path.extension().and_then(|ext| ext.to_str()) == Some("json") {


            let mut abi_file = File::open(&abi_path).expect("Unable to open ABI file");
            let mut abi_str = String::new();
            abi_file.read_to_string(&mut abi_str).expect("Unable to read ABI file");

            let abi_file: AbiFile = serde_json::from_str(&abi_str).expect("Invalid ABI JSON");
            let mut ts_output = String::new();

            for item in abi_file.abi.iter()  {
                
                match item["type"].as_str().unwrap() {
                    "function" => {
                        ts_output.push_str(&generate_ts_function_binding(item));
                    }
                    "struct" => {
                        ts_output.push_str(&generate_ts_struct_binding(item));
                    }
                    _ => {}
                }
            }

            let ts_output_path = abi_path.with_extension("ts");
            let mut file = File::create(ts_output_path).expect("Unable to create file");

            println!("Generated TypeScript bindings: {}", ts_output);
            file.write_all(ts_output.as_bytes())
                .expect("Unable to write TypeScript bindings");
        }
    }
    Ok(())
}


// We can add other bindings specific to the lang like this
fn generate_ts_function_binding(item: &Value) -> String {
    let mut binding = format!("export function {}(", item["name"].as_str().unwrap());
    let inputs = item["inputs"].as_array().unwrap();
    let outputs = item["outputs"].as_array().unwrap();

    for (i, input) in inputs.iter().enumerate() {
        if i > 0 {
            binding.push_str(", ");
        }
        binding.push_str(&format!("{}: {}", input["name"].as_str().unwrap(), map_rust_type_to_ts(input["type"].as_str().unwrap())));
    }

    binding.push_str("): ");
    
    if outputs.len() == 1 {
        binding.push_str(&map_rust_type_to_ts(outputs[0]["type"].as_str().unwrap()));
    } else {
        binding.push_str("void");
    }

    binding.push_str(" {\n  // TODO: Implement function\n}\n\n");
    binding
}

fn generate_ts_struct_binding(item: &Value) -> String {

    let struct_name = item["name"].as_str().unwrap().split("::").last().unwrap();
    let mut binding = format!("export interface {} {{\n", struct_name);
    let members = item["members"].as_array().unwrap();

    for member in members.iter() {
        binding.push_str(&format!("  {}: {};\n", member["name"].as_str().unwrap(), map_rust_type_to_ts(member["type"].as_str().unwrap())));
    }

    binding.push_str("}\n\n");
    binding
}

fn map_rust_type_to_ts(rust_type: &str) -> String {
    match rust_type {
        "core::felt252" => "number".to_string(),
        "core::integer::u32" => "number".to_string(),
        "core::integer::u32" => "number".to_string(),
        "core::array::Span::<core::felt252>" => "number[]".to_string(),
        _ => {
            if rust_type.starts_with("dojo_examples::") {
                rust_type.split("::").last().unwrap().to_string()
            } else {
                "any".to_string()
            }
        }
    }
}