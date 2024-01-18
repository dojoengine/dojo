use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, NamedPlugin, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

use super::unsupported_arg_diagnostic;

#[derive(Debug, Default)]
pub struct ArrayCapMacro;

impl NamedPlugin for ArrayCapMacro {
    const NAME: &'static str = "array_cap";
}

impl InlineMacroExprPlugin for ArrayCapMacro {
    fn generate_code(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        syntax: &ast::ExprInlineMacro,
    ) -> InlinePluginResult {
        let ast::WrappedArgList::ParenthesizedArgList(arg_list) = syntax.arguments(db) else {
            return unsupported_bracket_diagnostic(db, syntax);
        };

        let mut builder = PatchBuilder::new(db);

        builder.add_str("{");
        builder.add_str("let mut __array_with_cap__ = array![];");

        let args = arg_list.arguments(db).elements(db);

        if args.is_empty() || args.len() > 2 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"(capacity, (values,))\"".to_string(),
                }],
            };
        }

        let capacity = match (args[0]).as_syntax_node().get_text(db).parse::<usize>() {
            Ok(c) => c,
            Err(_) => {
                return InlinePluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                        message: "Invalid capacity, usize expected".to_string(),
                    }],
                };
            }
        };

        let bundle = if args.len() == 1 {
            let values: Vec<String> = vec!["0"; capacity].iter().map(|&s| s.to_string()).collect();
            values
        } else {
            // 2 args, we parse the user values and fill with 0 if necessary.
            let ast::ArgClause::Unnamed(values) = args[1].arg_clause(db) else {
                return unsupported_arg_diagnostic(db, syntax);
            };

            let mut bundle = vec![];

            match values.value(db) {
                ast::Expr::Tuple(list) => {
                    let mut i = 0;
                    for expr in list.expressions(db).elements(db).into_iter() {
                        if i > capacity {
                            return InlinePluginResult {
                                code: None,
                                diagnostics: vec![PluginDiagnostic {
                                    stable_ptr: expr.stable_ptr().untyped(),
                                    message: "Number of values is exceeded the capacity"
                                        .to_string(),
                                }],
                            };
                        }

                        let syntax_node = expr.as_syntax_node();
                        bundle.push(syntax_node.get_text(db));
                        i += 1;
                    }

                    if i < capacity {
                        for _ in i..capacity {
                            bundle.push("0".to_string());
                        }
                    }
                }
                _ => {
                    return InlinePluginResult {
                        code: None,
                        diagnostics: vec![PluginDiagnostic {
                            message: "Invalid arguments. Expected \"(capacity, (values,))\""
                                .to_string(),
                            stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                        }],
                    };
                }
            };

            bundle
        };

        for value in bundle {
            builder.add_str(&format!("__array_with_cap__.append({});", value));
        }

        builder.add_str("__array_with_cap__");
        builder.add_str("}");

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "array_cap_inline_macro".into(),
                content: builder.code,
                diagnostics_mappings: builder.diagnostics_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}
