use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use generator::r#enum::TsEnumGenerator;
use generator::function::TsFunctionGenerator;
use generator::interface::TsInterfaceGenerator;
use generator::models::TsModelsGenerator;
use generator::schema::TsSchemaGenerator;
use writer::{TsFileContractWriter, TsFileWriter};

use super::BindgenWriter;
use crate::error::BindgenResult;
use crate::plugins::BuiltinPlugin;
use crate::DojoData;

pub(crate) mod generator;
pub(crate) mod writer;

pub struct TypescriptPlugin {
    writers: Vec<Box<dyn BindgenWriter>>,
}

impl TypescriptPlugin {
    pub fn new() -> Self {
        Self {
            writers: vec![
                Box::new(TsFileWriter::new(
                    "models.gen.ts",
                    vec![
                        Box::new(TsInterfaceGenerator {}),
                        Box::new(TsEnumGenerator {}),
                        Box::new(TsSchemaGenerator {}),
                        Box::new(TsModelsGenerator {}),
                    ],
                )),
                Box::new(TsFileContractWriter::new(
                    "contracts.gen.ts",
                    vec![Box::new(TsFunctionGenerator {})],
                )),
            ],
        }
    }
}

#[async_trait]
impl BuiltinPlugin for TypescriptPlugin {
    async fn generate_code(&self, data: &DojoData) -> BindgenResult<HashMap<PathBuf, Vec<u8>>> {
        let mut out: HashMap<PathBuf, Vec<u8>> = HashMap::new();

        let code = self
            .writers
            .iter()
            .map(|writer| match writer.write(writer.get_path(), data) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to generate code for typescript plugin: {e}");
                    ("".into(), Vec::new())
                }
            })
            .collect::<Vec<_>>();

        code.iter().for_each(|(path, code)| {
            if code.is_empty() {
                return;
            }
            out.insert(PathBuf::from(path), code.clone());
        });

        Ok(out)
    }
}
