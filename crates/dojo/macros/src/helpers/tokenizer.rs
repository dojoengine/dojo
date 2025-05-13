use cairo_lang_macro::{quote, TextSpan, Token, TokenStream, TokenTree};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::with_db::SyntaxNodeWithDb;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

pub struct DojoTokenizer {}

/// DojoTokenizer provides some functions to build TokenStream or TokenTree.
impl DojoTokenizer {
    /// Convert a string into a TokenTree to be used in `quote!` macro.
    pub fn tokenize(s: &str) -> TokenTree {
        TokenTree::Ident(Token::new(s, TextSpan::call_site()))
    }

    /// In attribute proc macros, the tagged element is removed by default,
    /// so it has to be copied into the output token stream to be kept.
    /// At the same time, built-in derive attributes have already been processed
    /// before the processing of the proc macro but these derive attributes remain
    /// in the input token stream.
    /// To avoid processing them a second time, they have to be removed from the original
    /// element.
    pub fn rebuild_original_struct(
        db: &SimpleParserDatabase,
        struct_ast: &ast::ItemStruct,
    ) -> TokenStream {
        let el = struct_ast.visibility(db).as_syntax_node();
        let visibility = SyntaxNodeWithDb::new(&el, db);

        let el = struct_ast.name(db).as_syntax_node();
        let name = SyntaxNodeWithDb::new(&el, db);

        let el = struct_ast.generic_params(db).as_syntax_node();
        let generics = SyntaxNodeWithDb::new(&el, db);

        let el = struct_ast.members(db).as_syntax_node();
        let members = SyntaxNodeWithDb::new(&el, db);

        quote! {
            #visibility struct #name<#generics> {
                #members
            }
        }
    }
}
