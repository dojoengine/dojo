use cairo_lang_macro::{quote, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::{ast, db::SyntaxGroup, Terminal, TypedSyntaxNode};

use dojo_types::naming;

use crate::helpers::{DojoParser, DojoTokenizer, ProcMacroResultExt};

pub(crate) fn process(token_stream: TokenStream) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();

    if let Some(expr) = DojoParser::parse_inline_args(&db, &token_stream) {
        return process_ast(&db, &expr);
    }

    ProcMacroResult::fail(format!(
        "selector_from_tag: invalid parameter (arg: {token_stream})"
    ))
}

fn process_ast(db: &dyn SyntaxGroup, expr: &ast::ExprParenthesized) -> ProcMacroResult {
    if let ast::Expr::String(s) = expr.expr(db) {
        let tag = s.text(db).to_string().replace("\"", "");

        if !naming::is_valid_tag(&tag) {
            return ProcMacroResult::fail(
                "selector_from_tag: Invalid tag. Tag must be in the format of `namespace-name`."
                    .to_string(),
            );
        }

        let selector = naming::compute_selector_from_tag(&tag);
        let selector = format!("{:#64x}", selector);

        let token = DojoTokenizer::tokenize(&selector);
        return ProcMacroResult::new(quote! { #token });
    }

    ProcMacroResult::fail(format!(
        "selector_from_tag: invalid parameter type (arg: {})",
        expr.as_syntax_node().get_text(db)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo_lang_macro::{Severity, TokenStream};

    #[test]
    fn test_with_bad_inputs() {
        // input without parenthesis
        let input = "hello";
        let res = process(TokenStream::new(vec![DojoTokenizer::tokenize(input)]));

        assert_eq!(res.diagnostics.len(), 1);

        assert_eq!(res.diagnostics[0].severity, Severity::Error);
        assert_eq!(
            res.diagnostics[0].message,
            "selector_from_tag: invalid parameter (arg: hello)".to_string()
        );

        // bad input type
        let input = "(1234)";
        let res = process(TokenStream::new(vec![DojoTokenizer::tokenize(input)]));

        assert_eq!(res.diagnostics.len(), 1);

        assert_eq!(res.diagnostics[0].severity, Severity::Error);
        assert_eq!(
            res.diagnostics[0].message,
            "selector_from_tag: invalid parameter type (arg: (1234))".to_string()
        );
    }

    #[test]
    fn test_with_valid_input_but_invalid_tag() {
        let input = "(\"hello\")";

        let res = process(TokenStream::new(vec![DojoTokenizer::tokenize(input)]));

        assert_eq!(res.diagnostics.len(), 1);

        assert_eq!(res.diagnostics[0].severity, Severity::Error);
        assert_eq!(
            res.diagnostics[0].message,
            "selector_from_tag: Invalid tag. Tag must be in the format of `namespace-name`."
                .to_string()
        );
    }

    #[test]
    fn test_with_valid_input_and_valid_tag() {
        let input = "(\"ns-Position\")";
        let expected = "0x5e12c61e9cf30881c126a6d298975c8d79f95abed1a05c2d38b7803ed19445f";

        let res = process(TokenStream::new(vec![DojoTokenizer::tokenize(input)]));

        assert_eq!(res.diagnostics.len(), 0);
        assert_eq!(
            res.token_stream,
            TokenStream::new(vec![DojoTokenizer::tokenize(expected)])
        );
    }
}
