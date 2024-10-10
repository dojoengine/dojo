use crate::DojoData;
use cainome::parser::tokens::Composite;
use std::path::{Path, PathBuf};

use crate::{
    error::BindgenResult,
    plugins::{BindgenGenerator, BindgenWriter},
};

pub struct TsFileWriter {
    path: &'static str,
    generators: Vec<Box<dyn BindgenGenerator>>,
}

impl TsFileWriter {
    pub fn new(path: &'static str, generators: Vec<Box<dyn BindgenGenerator>>) -> Self {
        Self { path, generators }
    }
}

impl BindgenWriter for TsFileWriter {
    fn write(&self, path: &str, data: &DojoData) -> BindgenResult<(PathBuf, Vec<u8>)> {
        let models_path = Path::new(path).to_owned();
        let mut models = data.models.values().collect::<Vec<_>>();

        // Sort models based on their tag to ensure deterministic output.
        models.sort_by(|a, b| a.tag.cmp(&b.tag));
        let composites = models
            .iter()
            .map(|m| {
                let mut composites: Vec<&Composite> = Vec::new();
                let mut enum_composites =
                    m.tokens.enums.iter().map(|e| e.to_composite().unwrap()).collect::<Vec<_>>();
                let mut struct_composites =
                    m.tokens.structs.iter().map(|s| s.to_composite().unwrap()).collect::<Vec<_>>();
                let mut func_composites = m
                    .tokens
                    .functions
                    .iter()
                    .map(|f| f.to_composite().unwrap())
                    .collect::<Vec<_>>();
                composites.append(&mut enum_composites);
                composites.append(&mut struct_composites);
                composites.append(&mut func_composites);
                composites
            })
            .flatten()
            .filter(|c| !(c.type_path.starts_with("dojo::") || c.type_path.starts_with("core::")))
            .collect::<Vec<_>>();

        let code = self
            .generators
            .iter()
            .fold(Vec::new(), |mut acc, g| {
                composites.iter().for_each(|c| {
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
            })
            .join("\n");

        Ok((models_path, code.as_bytes().to_vec()))
    }

    fn get_path(&self) -> &'static str {
        self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DojoData, DojoWorld};
    use std::{collections::HashMap, path::PathBuf};

    #[test]
    fn test_ts_file_writer() {
        let writer = TsFileWriter::new("models.gen.ts", Vec::new());

        let data = DojoData {
            models: HashMap::new(),
            contracts: HashMap::new(),
            world: DojoWorld { name: "0x01".to_string() },
        };

        let (path, code) = writer.write("models.gen.ts", &data).unwrap();
        assert_eq!(path, PathBuf::from("models.gen.ts"));
        assert_eq!(code, Vec::<u8>::new());
    }
}
