use cairo_lang_macro::{Diagnostic, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::attribute::structured::{AttributeArgVariant, AttributeStructurize};
use cairo_lang_syntax::node::ast::{Attribute, Member as MemberAst};
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::kind::SyntaxKind::{ExprParenthesized, ItemModule, ItemStruct};
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

use crate::helpers::{DiagnosticsExt, Member};

pub struct DojoParser {}

/// DojoParser provides some functions to parse TokenStream/SyntaxNode.
impl DojoParser {
    /// Parse an input token stream and return a ItemStruct syntax node if found.
    pub(crate) fn parse_and_find_struct(
        db: &SimpleParserDatabase,
        token_stream: &TokenStream,
    ) -> Option<ast::ItemStruct> {
        let (root_node, _diagnostics) = db.parse_token_stream(token_stream);

        for n in root_node.descendants(db) {
            if n.kind(db) == ItemStruct {
                let struct_ast = ast::ItemStruct::from_syntax_node(db, n);
                return Some(struct_ast);
            }
        }

        None
    }

    /// Parse an input token stream and return a ItemModule syntax node if found.
    pub(crate) fn parse_and_find_module(
        db: &SimpleParserDatabase,
        token_stream: &TokenStream,
    ) -> Option<ast::ItemModule> {
        let (root_node, _diagnostics) = db.parse_token_stream(token_stream);

        for n in root_node.descendants(db) {
            if n.kind(db) == ItemModule {
                let module_ast = ast::ItemModule::from_syntax_node(db, n);
                return Some(module_ast);
            }
        }

        None
    }

    /// Parse the input token stream of an inline proc macro as a
    /// parenthesized expression.
    pub(crate) fn parse_inline_args(
        db: &SimpleParserDatabase,
        token_stream: &TokenStream,
    ) -> Option<ast::ExprParenthesized> {
        let (root_node, _diagnostics) = db.parse_token_stream_expr(token_stream);

        for n in root_node.descendants(db) {
            if n.kind(db) == ExprParenthesized {
                return Some(ast::ExprParenthesized::from_syntax_node(db, n));
            }
        }

        None
    }

    /// Parse a list of member syntax nodes into a list of `Member`.
    pub(crate) fn parse_members(
        db: &SimpleParserDatabase,
        members: &[MemberAst],
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<Member> {
        let mut parsing_keys = true;

        members
            .iter()
            .map(|member_ast| {
                let is_key = member_ast.has_attr(db, "key");

                let member = Member {
                    name: member_ast.name(db).text(db).to_string(),
                    ty: member_ast
                        .type_clause(db)
                        .ty(db)
                        .as_syntax_node()
                        .get_text(db)
                        .trim()
                        .to_string(),
                    key: is_key,
                };

                // Make sure all keys are before values in the model.
                if is_key && !parsing_keys {
                    diagnostics.push(Diagnostic::error(
                        "Key members must be defined before non-key members.",
                    ));
                    // Don't return here, since we don't want to stop processing the members after
                    // the first error to avoid diagnostics just because the
                    // field is missing.
                }

                parsing_keys &= is_key;

                member
            })
            .collect::<Vec<_>>()
    }

    /// Extracts the names of the derive attributes from the given attributes.
    ///
    /// # Examples
    ///
    /// Derive usage should look like this:
    ///
    /// ```no_run,ignore
    /// #[derive(Introspect)]
    /// struct MyStruct {}
    /// ```
    ///
    /// And this function will return `["Introspect"]`.
    pub fn extract_derive_attr_names(
        db: &SimpleParserDatabase,
        diagnostics: &mut Vec<Diagnostic>,
        attrs: Vec<Attribute>,
    ) -> Vec<String> {
        attrs
            .iter()
            .filter_map(|attr| {
                let args = attr.clone().structurize(db).args;
                if args.is_empty() {
                    diagnostics.push_error("Expected args.".into());
                    None
                } else {
                    Some(args.into_iter().filter_map(|a| {
                        if let AttributeArgVariant::Unnamed(ast::Expr::Path(path)) = a.variant {
                            if let [ast::PathSegment::Simple(segment)] = &path.elements(db)[..] {
                                Some(segment.ident(db).text(db).to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }))
                }
            })
            .flatten()
            .collect::<Vec<_>>()
    }
}
