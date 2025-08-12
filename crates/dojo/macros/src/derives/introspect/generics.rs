use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::Terminal;
use cairo_lang_syntax::node::ast::{GenericParam, OptionWrappedGenericParamList};
use itertools::Itertools;

// Extract generic type information and build the
// type and impl information to add to the generated introspect
pub fn build_generic_types(
    db: &SimpleParserDatabase,
    generic_params: OptionWrappedGenericParamList,
) -> Vec<String> {
    let generic_types =
        if let OptionWrappedGenericParamList::WrappedGenericParamList(params) = generic_params {
            params
                .generic_params(db)
                .elements(db)
                .filter_map(|el| {
                    if let GenericParam::Type(typ) = el {
                        Some(typ.name(db).text(db).to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

    generic_types
}

pub fn build_generic_impls(
    gen_types: &[String],
    base_impls: &[String],
    additional_impls: &[String],
) -> String {
    let mut gen_impls = gen_types
        .iter()
        .map(|g| {
            format!(
                "{g}, {base_impls}",
                base_impls = base_impls.iter().map(|i| format!("{i}<{g}>")).join(", ")
            )
        })
        .collect::<Vec<_>>();

    if !gen_types.is_empty() {
        gen_impls.extend(additional_impls.to_vec());
    }

    gen_impls.join(", ")
}
