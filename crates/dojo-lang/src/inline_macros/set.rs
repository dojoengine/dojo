use std::collections::HashMap;

use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, NamedPlugin, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::ast::{ExprPath, ExprStructCtorCall, FunctionWithBody, ItemModule};
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

use super::unsupported_arg_diagnostic;
use super::utils::{parent_of_kind, SystemRWOpRecord, SYSTEM_WRITES};

#[derive(Debug, Default)]
pub struct SetMacro;

impl NamedPlugin for SetMacro {
    const NAME: &'static str = "set";
    // Parents of set!()
    // -----------------
    // StatementExpr
    // StatementList
    // ExprBlock
    // FunctionWithBody
    // ImplItemList
    // ImplBody
    // ItemImpl
    // ItemList
    // ModuleBody
    // ItemModule
    // ItemList
    // SyntaxFile
}

impl InlineMacroExprPlugin for SetMacro {
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

        let args = arg_list.arguments(db).elements(db);

        if args.len() != 2 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"(world, (models,))\"".to_string(),
                }],
            };
        }

        let world = &args[0];

        let ast::ArgClause::Unnamed(models) = args[1].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };

        let mut bundle = vec![];

        match models.value(db) {
            ast::Expr::Parenthesized(parens) => {
                let syntax_node = parens.expr(db).as_syntax_node();
                bundle.push((syntax_node.get_text(db), syntax_node));
            }
            ast::Expr::Tuple(list) => {
                list.expressions(db).elements(db).into_iter().for_each(|expr| {
                    let syntax_node = expr.as_syntax_node();
                    bundle.push((syntax_node.get_text(db), syntax_node));
                })
            }
            ast::Expr::StructCtorCall(ctor) => {
                let syntax_node = ctor.as_syntax_node();
                bundle.push((syntax_node.get_text(db), syntax_node));
            }
            _ => {
                return InlinePluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        message: "Invalid arguments. Expected \"(world, (models,))\"".to_string(),
                        stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    }],
                };
            }
        }

        if bundle.is_empty() {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    message: "Invalid arguments: No models provided.".to_string(),
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                }],
            };
        }

        let module_syntax_node =
            parent_of_kind(db, &syntax.as_syntax_node(), SyntaxKind::ItemModule);
        let module_name = if let Some(module_syntax_node) = &module_syntax_node {
            let mod_ast = ItemModule::from_syntax_node(db, module_syntax_node.clone());
            mod_ast.name(db).as_syntax_node().get_text_without_trivia(db)
        } else {
            "".into()
        };

        let fn_syntax_node =
            parent_of_kind(db, &syntax.as_syntax_node(), SyntaxKind::FunctionWithBody);
        let fn_name = if let Some(fn_syntax_node) = &fn_syntax_node {
            let fn_ast = FunctionWithBody::from_syntax_node(db, fn_syntax_node.clone());
            fn_ast.declaration(db).name(db).as_syntax_node().get_text_without_trivia(db)
        } else {
            "".into()
        };

        for (entity, syntax_node) in bundle {
            // db.lookup_intern_file(key0);
            if !module_name.is_empty() && !fn_name.is_empty() {
                let mut system_writes = SYSTEM_WRITES.lock().unwrap();
                // fn_syntax_node
                if system_writes.get(&module_name).is_none() {
                    system_writes.insert(module_name.clone(), HashMap::new());
                }
                let fns = system_writes.get_mut(&module_name).unwrap();
                if fns.get(&fn_name).is_none() {
                    fns.insert(fn_name.clone(), vec![]);
                }

                match syntax_node.kind(db) {
                    SyntaxKind::ExprPath => {
                        fns.get_mut(&fn_name).unwrap().push(SystemRWOpRecord::Path(
                            ExprPath::from_syntax_node(db, syntax_node),
                        ));
                    }
                    // SyntaxKind::StatementExpr => {
                    //     todo!()
                    // }
                    SyntaxKind::ExprStructCtorCall => {
                        fns.get_mut(&fn_name).unwrap().push(SystemRWOpRecord::StructCtor(
                            ExprStructCtorCall::from_syntax_node(db, syntax_node.clone()),
                        ));
                    }
                    _ => eprintln!(
                        "Unsupport component value type {} for semantic writer analysis",
                        syntax_node.kind(db)
                    ),
                }
            }

            builder.add_str(&format!(
                "
                let __set_macro_value__ = {};
                {}.set_entity(dojo::model::Model::name(@__set_macro_value__),
                 dojo::model::Model::keys(@__set_macro_value__), 0_u8,
                 dojo::model::Model::values(@__set_macro_value__),
                 dojo::model::Model::layout(@__set_macro_value__));",
                entity,
                world.as_syntax_node().get_text(db),
            ));
        }
        builder.add_str("}");

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "set_inline_macro".into(),
                content: builder.code,
                diagnostics_mappings: builder.diagnostics_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}
