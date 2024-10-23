//! Derive macros.
//!
//! A derive macros is a macro that is used to generate code generally for a struct or enum.
//! The input of the macro consists of the AST of the struct or enum and the attributes of the derive macro.

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{PluginDiagnostic, PluginGeneratedFile, PluginResult};
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::attribute::structured::{AttributeArgVariant, AttributeStructurize};
use cairo_lang_syntax::node::ast::Attribute;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::ids::SyntaxStablePtrId;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

use introspect::{handle_introspect_enum, handle_introspect_struct};
use print::{handle_print_enum, handle_print_struct};

pub mod introspect;
pub mod print;

pub const DOJO_PRINT_DERIVE: &str = "Print";
pub const DOJO_INTROSPECT_DERIVE: &str = "Introspect";
pub const DOJO_PACKED_DERIVE: &str = "IntrospectPacked";

/// Handles all the dojo derives macro and returns the generated code and diagnostics.
pub fn dojo_derive_all(
    db: &dyn SyntaxGroup,
    attrs: Vec<Attribute>,
    item_ast: &ast::ModuleItem,
) -> PluginResult {
    if attrs.is_empty() {
        return PluginResult::default();
    }

    let mut diagnostics = vec![];

    let derive_attr_names = extract_derive_attr_names(db, &mut diagnostics, attrs);

    let (rewrite_nodes, derive_diagnostics) = handle_derive_attrs(db, &derive_attr_names, item_ast);

    diagnostics.extend(derive_diagnostics);

    let mut builder = PatchBuilder::new(db, item_ast);
    for node in rewrite_nodes {
        builder.add_modified(node);
    }

    let (code, code_mappings) = builder.build();

    let item_name = item_ast.as_syntax_node().get_text_without_trivia(db).into();

    PluginResult {
        code: Some(PluginGeneratedFile {
            name: item_name,
            content: code,
            aux_data: None,
            code_mappings,
        }),
        diagnostics,
        remove_original_item: false,
    }
}

/// Handles the derive attributes of a struct or enum.
pub fn handle_derive_attrs(
    db: &dyn SyntaxGroup,
    attrs: &[String],
    item_ast: &ast::ModuleItem,
) -> (Vec<RewriteNode>, Vec<PluginDiagnostic>) {
    let mut rewrite_nodes = Vec::new();
    let mut diagnostics = Vec::new();

    check_for_derive_attr_conflicts(&mut diagnostics, item_ast.stable_ptr().0, attrs);

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
            // Currently Dojo plugin doesn't support derive macros on other items than struct and enum.
            diagnostics.push(PluginDiagnostic {
                stable_ptr: item_ast.stable_ptr().0,
                message:
                    "Dojo plugin doesn't support derive macros on other items than struct and enum."
                        .to_string(),
                severity: Severity::Error,
            });
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
    diagnostics: &mut Vec<PluginDiagnostic>,
    attrs: Vec<Attribute>,
) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            let args = attr.clone().structurize(db).args;
            if args.is_empty() {
                diagnostics.push(PluginDiagnostic {
                    stable_ptr: attr.stable_ptr().0,
                    message: "Expected args.".into(),
                    severity: Severity::Error,
                });
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
fn check_for_derive_attr_conflicts(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: SyntaxStablePtrId,
    attr_names: &[String],
) {
    if attr_names.contains(&DOJO_INTROSPECT_DERIVE.to_string())
        && attr_names.contains(&DOJO_PACKED_DERIVE.to_string())
    {
        diagnostics.push(PluginDiagnostic {
            stable_ptr: diagnostic_item,
            message: format!(
                "{} and {} attributes cannot be used at a same time.",
                DOJO_INTROSPECT_DERIVE, DOJO_PACKED_DERIVE
            ),
            severity: Severity::Error,
        });
    }
}
