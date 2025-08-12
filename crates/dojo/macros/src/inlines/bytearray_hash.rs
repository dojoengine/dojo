use cairo_lang_macro::{quote, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_types::naming;

use crate::helpers::{DojoParser, DojoTokenizer, ProcMacroResultExt};

pub(crate) fn process(token_stream: TokenStream) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();

    if let Some(expr) = DojoParser::parse_inline_args(&db, &token_stream) {
        return process_ast(&db, &expr);
    }

    ProcMacroResult::fail(format!("bytearray_hash: invalid parameter (arg: {token_stream})"))
}

fn process_ast(db: &dyn SyntaxGroup, expr: &ast::ExprParenthesized) -> ProcMacroResult {
    if let ast::Expr::String(s) = expr.expr(db) {
        let input = s.text(db).to_string().replace("\"", "");
        let hash = naming::compute_bytearray_hash(&input);
        let hash = format!("{:#64x}", hash);

        let token = DojoTokenizer::tokenize(&hash);
        return ProcMacroResult::new(quote! { #token });
    }

    ProcMacroResult::fail(format!(
        "bytearray_hash: invalid parameter type (arg: {})",
        expr.as_syntax_node().get_text_without_all_comment_trivia(db)
    ))
}

#[cfg(test)]
mod tests {
    use cairo_lang_macro::{Severity, TokenStream};

    use super::*;

    #[test]
    fn test_with_bad_inputs() {
        // input without parenthesis
        let input = "hello";
        let res = process(TokenStream::new(vec![DojoTokenizer::tokenize(input)]));

        assert_eq!(res.diagnostics.len(), 1);

        assert_eq!(res.diagnostics[0].severity, Severity::Error);
        assert_eq!(
            res.diagnostics[0].message,
            "bytearray_hash: invalid parameter (arg: hello)".to_string()
        );

        // bad input type
        let input = "(1234)";
        let res = process(TokenStream::new(vec![DojoTokenizer::tokenize(input)]));

        assert_eq!(res.diagnostics.len(), 1);

        assert_eq!(res.diagnostics[0].severity, Severity::Error);
        assert_eq!(
            res.diagnostics[0].message,
            "bytearray_hash: invalid parameter type (arg: (1234))".to_string()
        );
    }

    #[test]
    fn test_with_valid_input() {
        let input = "(\"hello\")";
        let expected = "0x30c616199236e6ead87ae6931d750794c9702f3604ed1c1006f60f01d129aea";

        let res = process(TokenStream::new(vec![DojoTokenizer::tokenize(input)]));

        assert_eq!(res.diagnostics.len(), 0);
        assert_eq!(res.token_stream, TokenStream::new(vec![DojoTokenizer::tokenize(expected)]));
    }
}
