use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{PluginDiagnostic, PluginGeneratedFile, PluginResult};
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, ids, Terminal, TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

use crate::syntax::self_param;
use crate::syntax::world_param::{self, WorldParamInjectionKind};

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

    /// Rewrites parameter list by adding `self` parameter based on the `world` parameter.
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

        self_param::check_parameter(db, &param_list, diagnostic_item, &mut self.diagnostics);

        let world_injection = world_param::parse_world_injection(
            db,
            param_list,
            diagnostic_item,
            &mut self.diagnostics,
        );

        match world_injection {
            WorldParamInjectionKind::None => {
                params.insert(0, "self: @TContractState".to_string());
            }
            WorldParamInjectionKind::View => {
                params.remove(0);
                params.insert(0, "self: @TContractState".to_string());
            }
            WorldParamInjectionKind::External => {
                params.remove(0);
                params.insert(0, "ref self: TContractState".to_string());
            }
        };

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

        let params_str = self.rewrite_parameters(
            db,
            fn_ast.declaration(db).signature(db).parameters(db),
            fn_ast.stable_ptr().untyped(),
        );

        rewritten_params.set_str(params_str);
        vec![rewritten_fn]
    }
}
