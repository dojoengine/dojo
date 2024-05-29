use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{PluginDiagnostic, PluginGeneratedFile, PluginResult};
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, ids, Terminal, TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

#[derive(Debug)]
pub struct DojoInterface {
    diagnostics: Vec<PluginDiagnostic>,
}

impl DojoInterface {
    pub fn from_trait(db: &dyn SyntaxGroup, trait_ast: ast::ItemTrait) -> PluginResult {
        let name = trait_ast.name(db).text(db);
        let mut system = DojoInterface { diagnostics: vec![] };
        let mut builder = PatchBuilder::new(db, &trait_ast);

        if let ast::MaybeTraitBody::Some(body) = trait_ast.body(db) {
            let body_nodes: Vec<_> = body
                .items(db)
                .elements(db)
                .iter()
                .flat_map(|el| {
                    if let ast::TraitItem::Function(fn_ast) = el {
                        return system.rewrite_function(db, fn_ast.clone());
                    }

                    system.diagnostics.push(PluginDiagnostic {
                        stable_ptr: el.stable_ptr().untyped(),
                        message: "Anything other than functions is not supported in a \
                                  dojo::interface"
                            .to_string(),
                        severity: Severity::Error,
                    });

                    vec![]
                })
                .collect();

            builder.add_modified(RewriteNode::interpolate_patched(
                "
                #[starknet::interface]
                trait $name$<TContractState> {
                    $body$
                }
                ",
                &UnorderedHashMap::from([
                    ("name".to_string(), RewriteNode::Text(name.to_string())),
                    ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                ]),
            ));
        } else {
            // empty trait
            builder.add_modified(RewriteNode::interpolate_patched(
                "
                #[starknet::interface]
                trait $name$<TContractState> {}
                ",
                &UnorderedHashMap::from([(
                    "name".to_string(),
                    RewriteNode::Text(name.to_string()),
                )]),
            ));
        }

        let (code, code_mappings) = builder.build();

        PluginResult {
            code: Some(PluginGeneratedFile {
                name: name.clone(),
                content: code,
                aux_data: None,
                code_mappings,
            }),
            diagnostics: system.diagnostics,
            remove_original_item: true,
        }
    }

    /// Rewrites parameter list  by adding `self` parameter if missing.
    ///
    /// Reports an error in case of `ref self` as systems are supposed to be 100% stateless.
    pub fn rewrite_parameters(
        &mut self,
        db: &dyn SyntaxGroup,
        param_list: ast::ParamList,
        diagnostic_item: ids::SyntaxStablePtrId,
    ) -> String {
        let mut params = param_list
            .elements(db)
            .iter()
            .map(|e| e.as_syntax_node().get_text(db))
            .collect::<Vec<_>>();

        let mut need_to_add_self = true;
        if !params.is_empty() {
            let first_param = param_list.elements(db)[0].clone();
            let param_name = first_param.name(db).text(db).to_string();

            if param_name.eq(&"self".to_string()) {
                let param_modifiers = first_param
                    .modifiers(db)
                    .elements(db)
                    .iter()
                    .map(|e| e.as_syntax_node().get_text(db).trim().to_string())
                    .collect::<Vec<_>>();

                let param_type = first_param
                    .type_clause(db)
                    .ty(db)
                    .as_syntax_node()
                    .get_text(db)
                    .trim()
                    .to_string();

                if param_modifiers.contains(&"ref".to_string())
                    && param_type.eq(&"TContractState".to_string())
                {
                    self.diagnostics.push(PluginDiagnostic {
                        stable_ptr: diagnostic_item,
                        message: "Functions of dojo::interface cannot have `ref self` parameter."
                            .to_string(),
                        severity: Severity::Error,
                    });

                    need_to_add_self = false;
                }

                if param_type.eq(&"@TContractState".to_string()) {
                    need_to_add_self = false;
                }
            }
        };

        if need_to_add_self {
            params.insert(0, "self: @TContractState".to_string());
        }

        params.join(", ")
    }

    /// Rewrites function declaration by adding `self` parameter if missing,
    pub fn rewrite_function(
        &mut self,
        db: &dyn SyntaxGroup,
        fn_ast: ast::TraitItemFunction,
    ) -> Vec<RewriteNode> {
        let mut rewritten_fn = RewriteNode::from_ast(&fn_ast);
        let rewritten_params = rewritten_fn
            .modify_child(db, ast::TraitItemFunction::INDEX_DECLARATION)
            .modify_child(db, ast::FunctionDeclaration::INDEX_SIGNATURE)
            .modify_child(db, ast::FunctionSignature::INDEX_PARAMETERS);

        rewritten_params.set_str(self.rewrite_parameters(
            db,
            fn_ast.declaration(db).signature(db).parameters(db),
            fn_ast.stable_ptr().untyped(),
        ));
        vec![rewritten_fn]
    }
}
