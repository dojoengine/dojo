use std::path::{Path, PathBuf};

use cainome::parser::tokens::Composite;
use dojo_world::contracts::naming;

use crate::error::BindgenResult;
use crate::plugins::{BindgenContractGenerator, BindgenModelGenerator, BindgenWriter, Buffer};
use crate::DojoData;

pub struct TsFileWriter {
    path: &'static str,
    generators: Vec<Box<dyn BindgenModelGenerator>>,
}

impl TsFileWriter {
    pub fn new(path: &'static str, generators: Vec<Box<dyn BindgenModelGenerator>>) -> Self {
        Self { path, generators }
    }
}

/// Helper function to filter out "Value" types only when base type exists.
/// Since Dojo auto generates a `ModelValue`, it is not required for the client to have both
/// the model and the model value.
/// However, we need to keep any type that might end with "Value" but not being used as a model.
fn filter_value_duplicates(composites: Vec<&Composite>) -> Vec<&Composite> {
    let type_names: std::collections::HashSet<String> =
        composites.iter().map(|c| c.type_name().to_string()).collect();

    composites
        .into_iter()
        .filter(|c| {
            let name = c.type_name();
            !(name.ends_with("Value") && type_names.contains(&name[..name.len() - 5]))
        })
        .collect()
}

impl BindgenWriter for TsFileWriter {
    fn write(&self, path: &str, data: &DojoData) -> BindgenResult<(PathBuf, Vec<u8>)> {
        let models_path = Path::new(path).to_owned();
        let models = data.models.values().collect::<Vec<_>>();
        let events = data.events.values().collect::<Vec<_>>();

        let mut e_composites = events
            .iter()
            .flat_map(|e| {
                e.tokens
                    .enums
                    .iter()
                    .map(|t| t.to_composite().unwrap())
                    .chain(e.tokens.structs.iter().map(|t| t.to_composite().unwrap()))
                    .chain(e.tokens.functions.iter().map(|t| t.to_composite().unwrap()))
            })
            .filter(|c| !(c.type_path.starts_with("dojo::") || c.type_path.starts_with("core::")))
            .collect::<Vec<_>>();

        let mut m_composites = models
            .iter()
            .flat_map(|m| {
                m.tokens
                    .enums
                    .iter()
                    .map(|t| t.to_composite().unwrap())
                    .chain(m.tokens.structs.iter().map(|t| t.to_composite().unwrap()))
                    .chain(m.tokens.functions.iter().map(|t| t.to_composite().unwrap()))
            })
            .filter(|c| !(c.type_path.starts_with("dojo::") || c.type_path.starts_with("core::")))
            .collect::<Vec<_>>();

        // Apply smart "Value" filtering
        e_composites = filter_value_duplicates(e_composites);
        m_composites = filter_value_duplicates(m_composites);

        // Sort models based on their tag to ensure deterministic output.
        // models.sort_by(|a, b| a.tag.cmp(&b.tag));
        m_composites.sort_by(|a, b| a.type_path.cmp(&b.type_path));
        e_composites.sort_by(|a, b| a.type_path.cmp(&b.type_path));

        // Store the world name at the beginning of the buffer as a special marker
        let mut initial_buffer = Buffer::new();
        initial_buffer.push(format!("// WORLD_NAME:{}", data.world.name));

        let mut code_buffer = self.generators.iter().fold(initial_buffer, |mut acc, g| {
            [m_composites.clone(), e_composites.clone()].concat().iter().for_each(|c| {
                match g.generate(c, &mut acc) {
                    Ok(code) => {
                        if !code.is_empty() {
                            acc.push(code)
                        }
                    }
                    Err(_e) => {
                        log::error!("Failed to generate code for model {}", c.type_path);
                    }
                };
            });
            acc
        });

        // Remove the world name marker from the final output
        code_buffer.retain(|s| !s.starts_with("// WORLD_NAME:"));

        let code = code_buffer.join("\n");

        Ok((models_path, code.as_bytes().to_vec()))
    }

    fn get_path(&self) -> &'static str {
        self.path
    }
}

pub struct TsFileContractWriter {
    path: &'static str,
    generators: Vec<Box<dyn BindgenContractGenerator>>,
}

impl TsFileContractWriter {
    pub fn new(path: &'static str, generators: Vec<Box<dyn BindgenContractGenerator>>) -> Self {
        Self { path, generators }
    }
}

impl BindgenWriter for TsFileContractWriter {
    fn write(&self, path: &str, data: &DojoData) -> BindgenResult<(PathBuf, Vec<u8>)> {
        let models_path = Path::new(path).to_owned();
        let mut functions = data
            .contracts
            .values()
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|c| {
                c.systems
                    .clone()
                    .into_iter()
                    .filter(|s| {
                        let name = s.to_function().unwrap().name.as_str();
                        ![
                            "contract_name",
                            "namespace",
                            "tag",
                            "name_hash",
                            "selector",
                            "dojo_init",
                            "namespace_hash",
                            "world",
                            "dojo_name",
                            "upgrade",
                            "world_dispatcher",
                        ]
                        .contains(&name)
                    })
                    .map(|s| match s.to_function() {
                        Ok(f) => (c, f.clone()),
                        Err(_) => {
                            panic!("Failed to get function out of system {:?}", &s)
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        functions.sort_by(|(ca, af), (cb, bf)| {
            let contract_a = naming::get_name_from_tag(&ca.tag);
            let contract_b = naming::get_name_from_tag(&cb.tag);
            let function_a = format!("{}_{}", contract_a, af.name);
            let function_b = format!("{}_{}", contract_b, bf.name);

            function_a.cmp(&function_b)
        });

        let code = self
            .generators
            .iter()
            .fold(Buffer::new(), |mut acc, g| {
                functions.iter().for_each(|(c, f)| {
                    match g.generate(c, f, &mut acc) {
                        Ok(code) => {
                            if !code.is_empty() {
                                acc.push(code)
                            }
                        }
                        Err(_) => {
                            log::error!("Failed to generate code for function {:?}", f.name);
                        }
                    };
                });

                acc
            })
            .join("\n");
        Ok((models_path, code.as_bytes().to_vec()))
    }
    fn get_path(&self) -> &str {
        self.path
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::{DojoData, DojoWorld};

    #[test]
    fn test_ts_file_writer() {
        let writer = TsFileWriter::new("models.gen.ts", Vec::new());

        let data = DojoData {
            models: HashMap::new(),
            contracts: HashMap::new(),
            world: DojoWorld { name: "0x01".to_string() },
            events: HashMap::new(),
        };

        let (path, code) = writer.write("models.gen.ts", &data).unwrap();
        assert_eq!(path, PathBuf::from("models.gen.ts"));
        assert_eq!(code, Vec::<u8>::new());
    }
}
