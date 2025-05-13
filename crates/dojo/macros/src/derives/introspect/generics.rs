use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{GenericParam, OptionWrappedGenericParamList};
use cairo_lang_syntax::node::Terminal;

// Extract generic type information and build the
// type and impl information to add to the generated introspect
pub(crate) fn build_generic_types_and_impls(
    db: &SimpleParserDatabase,
    generic_params: OptionWrappedGenericParamList,
) -> (Vec<String>, String) {
    let generic_types =
        if let OptionWrappedGenericParamList::WrappedGenericParamList(params) = generic_params {
            params
                .generic_params(db)
                .elements(db)
                .iter()
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

    let generic_impls = generic_types
        .iter()
        .map(|g| format!("{g}, impl {g}Introspect: dojo::meta::introspect::Introspect<{g}>"))
        .collect::<Vec<_>>()
        .join(", ");

    (generic_types, generic_impls)
}
