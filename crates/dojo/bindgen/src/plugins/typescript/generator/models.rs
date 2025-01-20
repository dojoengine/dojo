use cainome::parser::tokens::Composite;

use super::get_namespace_and_path;
use crate::error::BindgenResult;
use crate::plugins::{BindgenModelGenerator, Buffer};

pub(crate) struct TsModelsGenerator;

impl BindgenModelGenerator for TsModelsGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Buffer) -> BindgenResult<String> {
        let (ns, _namespace, type_name) = get_namespace_and_path(token);
        let models_mapping = "export enum ModelsMapping {";
        if !buffer.has(models_mapping) {
            buffer.push(format!(
                "export enum ModelsMapping {{\n\t{type_name} = '{ns}-{type_name}',\n}}",
            ));
            return Ok("".to_owned());
        }

        let gen = format!("\n\t{type_name} = '{ns}-{type_name}',");
        if buffer.has(&gen) {
            return Ok("".to_owned());
        }

        buffer.insert_after(gen, models_mapping, ",", 1);

        Ok("".to_owned())
    }
}
