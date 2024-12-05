use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_macro::{inline_macro, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::kind::SyntaxKind::ItemInlineMacro;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use dojo_types::naming;

use crate::proc_macro_result_ext::ProcMacroResultExt;

#[inline_macro]
pub fn selector_from_tag(token_stream: TokenStream) -> ProcMacroResult {
    handle_selector_from_tag_macro(token_stream)
}

pub fn handle_selector_from_tag_macro(token_stream: TokenStream) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();
    let (root_node, _diagnostics) = db.parse_virtual_with_diagnostics(token_stream);

    for n in root_node.descendants(&db) {
        if n.kind(&db) == ItemInlineMacro {
            let node = ast::ItemInlineMacro::from_syntax_node(&db, n);

            let ast::WrappedArgList::ParenthesizedArgList(arg_list) = node.arguments(&db) else {
                return ProcMacroResult::error(
                    "Macro `selector_from_tag!` does not support this bracket type.".to_string(),
                );
            };

            let args = arg_list.arguments(&db).elements(&db);

            if args.len() != 1 {
                return ProcMacroResult::error(
                    "Invalid arguments. Expected \"selector_from_tag!(\"tag\")\"".to_string(),
                );
            }

            let tag = &args[0].as_syntax_node().get_text(&db).replace('\"', "");

            if !naming::is_valid_tag(tag) {
                return ProcMacroResult::error(
                    "Invalid tag. Tag must be in the format of `namespace-name`.".to_string(),
                );
            }

            let selector = naming::compute_selector_from_tag(tag);

            let mut builder = PatchBuilder::new(&db, &node);
            builder.add_str(&format!("{:#64x}", selector));

            let (code, _) = builder.build();

            return ProcMacroResult::new(TokenStream::new(code));
        }
    }

    ProcMacroResult::error(
        "Macro `selector_from_tag!` must be called with a string parameter".to_string(),
    )
}
