//! Derive macros.
//!
//! A derive macros is a macro that is used to generate code generally for a struct or enum.
//! The input of the macro consists of the AST of the struct or enum and the attributes of the
//! derive macro.

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_macro::{derive_macro, Diagnostic, Diagnostics, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::attribute::structured::{AttributeArgVariant, AttributeStructurize};
use cairo_lang_syntax::node::ast::Attribute;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::kind::SyntaxKind::{ItemEnum, ItemStruct};
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use introspect::{handle_introspect_enum, handle_introspect_struct};
use print::{handle_print_enum, handle_print_struct};

use crate::diagnostic_ext::DiagnosticsExt;

pub mod introspect;
pub mod print;

pub const DOJO_PRINT_DERIVE: &str = "Print";
pub const DOJO_INTROSPECT_DERIVE: &str = "Introspect";
pub const DOJO_PACKED_DERIVE: &str = "IntrospectPacked";

#[derive_macro]
fn introspect(token_stream: TokenStream) -> ProcMacroResult {
    handle_derives_macros(token_stream)
}

#[derive_macro]
fn introspect_packed(token_stream: TokenStream) -> ProcMacroResult {
    handle_derives_macros(token_stream)
}

pub fn handle_derives_macros(token_stream: TokenStream) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();
    let (syn_file, _diagnostics) = db.parse_virtual_with_diagnostics(token_stream);

    for n in syn_file.descendants(&db) {
        // Process only the first module expected to be the contract.
        return match n.kind(&db) {
            ItemStruct => {
                let struct_ast = ast::ItemStruct::from_syntax_node(&db, n);
                let attrs = struct_ast.attributes(&db).query_attr(&db, "derive");

                dojo_derive_all(&db, attrs, &ast::ModuleItem::Struct(struct_ast))
            }
            ItemEnum => {
                let enum_ast = ast::ItemEnum::from_syntax_node(&db, n);
                let attrs = enum_ast.attributes(&db).query_attr(&db, "derive");

                dojo_derive_all(&db, attrs, &ast::ModuleItem::Enum(enum_ast))
            }
            _ => {
                continue;
            }
        };
    }

    ProcMacroResult::new(TokenStream::empty())
}

/// Handles all the dojo derives macro and returns the generated code and diagnostics.
pub fn dojo_derive_all(
    db: &dyn SyntaxGroup,
    attrs: Vec<Attribute>,
    item_ast: &ast::ModuleItem,
) -> ProcMacroResult {
    if attrs.is_empty() {
        return ProcMacroResult::new(TokenStream::empty());
    }

    let mut diagnostics = vec![];

    let derive_attr_names = extract_derive_attr_names(db, &mut diagnostics, attrs);

    let (rewrite_nodes, derive_diagnostics) = handle_derive_attrs(db, &derive_attr_names, item_ast);

    diagnostics.extend(derive_diagnostics);

    let mut builder = PatchBuilder::new(db, item_ast);
    for node in rewrite_nodes {
        builder.add_modified(node);
    }

    let (code, _) = builder.build();

    let item_name = item_ast.as_syntax_node().get_text_without_trivia(db).to_string();

    crate::debug_expand(&format!("DERIVE {}", item_name), &code.to_string());

    ProcMacroResult::new(TokenStream::new(code)).with_diagnostics(Diagnostics::new(diagnostics))
}

/// Handles the derive attributes of a struct or enum.
pub fn handle_derive_attrs(
    db: &dyn SyntaxGroup,
    attrs: &[String],
    item_ast: &ast::ModuleItem,
) -> (Vec<RewriteNode>, Vec<Diagnostic>) {
    let mut rewrite_nodes = Vec::new();
    let mut diagnostics = Vec::new();

    check_for_derive_attr_conflicts(&mut diagnostics, attrs);

    match item_ast {
        ast::ModuleItem::Struct(struct_ast) => {
            for a in attrs {
                match a.as_str() {
                    DOJO_PRINT_DERIVE => {
                        rewrite_nodes.push(handle_print_struct(db, struct_ast.clone()));
                    }
                    DOJO_INTROSPECT_DERIVE => {
                        rewrite_nodes.push(handle_introspect_struct(
                            db,
                            &mut diagnostics,
                            struct_ast.clone(),
                            false,
                        ));
                    }
                    DOJO_PACKED_DERIVE => {
                        rewrite_nodes.push(handle_introspect_struct(
                            db,
                            &mut diagnostics,
                            struct_ast.clone(),
                            true,
                        ));
                    }
                    _ => continue,
                }
            }
        }
        ast::ModuleItem::Enum(enum_ast) => {
            for a in attrs {
                match a.as_str() {
                    DOJO_PRINT_DERIVE => {
                        rewrite_nodes.push(handle_print_enum(db, enum_ast.clone()));
                    }
                    DOJO_INTROSPECT_DERIVE => {
                        rewrite_nodes.push(handle_introspect_enum(
                            db,
                            &mut diagnostics,
                            enum_ast.clone(),
                            false,
                        ));
                    }
                    DOJO_PACKED_DERIVE => {
                        rewrite_nodes.push(handle_introspect_enum(
                            db,
                            &mut diagnostics,
                            enum_ast.clone(),
                            true,
                        ));
                    }
                    _ => continue,
                }
            }
        }
        _ => {
            // Currently Dojo plugin doesn't support derive macros on other items than struct and
            // enum.
            diagnostics.push_error(
                "Dojo plugin doesn't support derive macros on other items than struct and enum."
                    .to_string(),
            );
        }
    }

    (rewrite_nodes, diagnostics)
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
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<Diagnostic>,
    attrs: Vec<Attribute>,
) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            let args = attr.clone().structurize(db).args;
            if args.is_empty() {
                diagnostics.push_error("Expected args.".to_string());
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

/// Checks for conflicts between introspect and packed attributes.
///
/// Introspect and IntrospectPacked cannot be used at a same time.
fn check_for_derive_attr_conflicts(diagnostics: &mut Vec<Diagnostic>, attr_names: &[String]) {
    if attr_names.contains(&DOJO_INTROSPECT_DERIVE.to_string())
        && attr_names.contains(&DOJO_PACKED_DERIVE.to_string())
    {
        diagnostics.push_error(format!(
            "{} and {} attributes cannot be used at a same time.",
            DOJO_INTROSPECT_DERIVE, DOJO_PACKED_DERIVE
        ));
    }
}
