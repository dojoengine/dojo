use std::collections::HashMap;

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, ids, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::system::Dependency;

use crate::plugin::{DojoAuxData, SystemAuxData};

pub struct DojoContract {
    diagnostics: Vec<PluginDiagnostic>,
    dependencies: HashMap<smol_str::SmolStr, Dependency>,
}

impl DojoContract {
    pub fn from_module(db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
        let name = module_ast.name(db).text(db);
        let mut system = DojoContract { diagnostics: vec![], dependencies: HashMap::new() };
        let mut has_event = false;
        let mut has_storage = false;

        if let MaybeModuleBody::Some(body) = module_ast.body(db) {
            let mut body_nodes: Vec<_> = body
                .items(db)
                .elements(db)
                .iter()
                .flat_map(|el| {
                    if let ast::ModuleItem::Enum(enum_ast) = el {
                        if enum_ast.name(db).text(db).to_string() == "Event" {
                            has_event = true;
                            return system.merge_event(db, enum_ast.clone());
                        }
                    } else if let ast::ModuleItem::Struct(struct_ast) = el {
                        if struct_ast.name(db).text(db).to_string() == "Storage" {
                            has_storage = true;
                            return system.merge_storage(db, struct_ast.clone());
                        }
                    } else if let ast::ModuleItem::Impl(impl_ast) = el {
                        // If an implementation is not targetting the ContractState,
                        // the auto injection of self and world is not applied.
                        let trait_path = impl_ast.trait_path(db).node.get_text(db);
                        if trait_path.contains("<ContractState>") {
                            return system.rewrite_impl(db, impl_ast.clone());
                        }
                    }

                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            if !has_event {
                body_nodes.append(&mut system.create_event())
            }

            if !has_storage {
                body_nodes.append(&mut system.create_storage())
            }

            let mut builder = PatchBuilder::new(db);
            builder.add_modified(RewriteNode::interpolate_patched(
                "
                #[starknet::contract]
                mod $name$ {
                    use dojo::world;
                    use dojo::world::IWorldDispatcher;
                    use dojo::world::IWorldDispatcherTrait;
                    use dojo::world::IWorldProvider;
                    use dojo::world::IDojoResourceProvider;
                   
                    component!(path: dojo::components::upgradeable::upgradeable, storage: \
                 upgradeable, event: UpgradeableEvent);

                    #[abi(embed_v0)]
                    impl DojoResourceProviderImpl of IDojoResourceProvider<ContractState> {
                        fn dojo_resource(self: @ContractState) -> felt252 {
                            '$name$'
                        }
                    }

                    #[abi(embed_v0)]
                    impl WorldProviderImpl of IWorldProvider<ContractState> {
                        fn world(self: @ContractState) -> IWorldDispatcher {
                            self.world_dispatcher.read()
                        }
                    }

                    #[abi(embed_v0)]
                    impl UpgradableImpl = \
                 dojo::components::upgradeable::upgradeable::UpgradableImpl<ContractState>;

                    $body$
                }
                ",
                &UnorderedHashMap::from([
                    ("name".to_string(), RewriteNode::Text(name.to_string())),
                    ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                ]),
            ));

            return PluginResult {
                code: Some(PluginGeneratedFile {
                    name: name.clone(),
                    content: builder.code,
                    aux_data: Some(DynGeneratedFileAuxData::new(DojoAuxData {
                        models: vec![],
                        systems: vec![SystemAuxData {
                            name,
                            dependencies: system.dependencies.values().cloned().collect(),
                        }],
                    })),
                    code_mappings: builder.code_mappings,
                }),
                diagnostics: system.diagnostics,
                remove_original_item: true,
            };
        }

        PluginResult::default()
    }

    pub fn merge_event(
        &mut self,
        db: &dyn SyntaxGroup,
        enum_ast: ast::ItemEnum,
    ) -> Vec<RewriteNode> {
        let mut rewrite_nodes = vec![];

        let elements = enum_ast.variants(db).elements(db);

        let variants = elements.iter().map(|e| e.as_syntax_node().get_text(db)).collect::<Vec<_>>();
        let variants = variants.join(",\n");

        rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            #[event]
            #[derive(Drop, starknet::Event)]
            enum Event {
                UpgradeableEvent: dojo::components::upgradeable::upgradeable::Event,
                $variants$
            }
            ",
            &UnorderedHashMap::from([("variants".to_string(), RewriteNode::Text(variants))]),
        ));
        rewrite_nodes
    }

    pub fn create_event(&mut self) -> Vec<RewriteNode> {
        vec![RewriteNode::Text(
            "
            #[event]
            #[derive(Drop, starknet::Event)]
            enum Event {
                UpgradeableEvent: dojo::components::upgradeable::upgradeable::Event,
            }
            "
            .to_string(),
        )]
    }

    pub fn merge_storage(
        &mut self,
        db: &dyn SyntaxGroup,
        struct_ast: ast::ItemStruct,
    ) -> Vec<RewriteNode> {
        let mut rewrite_nodes = vec![];

        let elements = struct_ast.members(db).elements(db);

        let members = elements.iter().map(|e| e.as_syntax_node().get_text(db)).collect::<Vec<_>>();
        let members = members.join(",\n");

        rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            #[storage]
            struct Storage {
                world_dispatcher: IWorldDispatcher,
                #[substorage(v0)]
                upgradeable: dojo::components::upgradeable::upgradeable::Storage,
                $members$
            }
            ",
            &UnorderedHashMap::from([("members".to_string(), RewriteNode::Text(members))]),
        ));
        rewrite_nodes
    }

    pub fn create_storage(&mut self) -> Vec<RewriteNode> {
        vec![RewriteNode::Text(
            "
            #[storage]
            struct Storage {
                world_dispatcher: IWorldDispatcher,
                #[substorage(v0)]
                upgradeable: dojo::components::upgradeable::upgradeable::Storage,
            }
            "
            .to_string(),
        )]
    }

    /// Gets name, modifiers and type from a function parameter.
    pub fn get_parameter_info(
        &mut self,
        db: &dyn SyntaxGroup,
        param: ast::Param,
    ) -> (String, String, String) {
        let name = param.name(db).text(db).trim().to_string();
        let modifiers = param.modifiers(db).as_syntax_node().get_text(db).trim().to_string();
        let param_type =
            param.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string();

        (name, modifiers, param_type)
    }

    /// Check if the function has a self parameter.
    ///
    /// Returns
    ///  * a boolean indicating if `self` has to be added,
    //   * a boolean indicating if there is a `ref self` parameter.
    pub fn check_self_parameter(
        &mut self,
        db: &dyn SyntaxGroup,
        param_list: ast::ParamList,
    ) -> (bool, bool) {
        let mut add_self = true;
        let mut has_ref_self = false;
        if !param_list.elements(db).is_empty() {
            let (param_name, param_modifiers, param_type) =
                self.get_parameter_info(db, param_list.elements(db)[0].clone());

            if param_name.eq(&"self".to_string()) {
                if param_modifiers.contains(&"ref".to_string())
                    && param_type.eq(&"ContractState".to_string())
                {
                    has_ref_self = false;
                    add_self = false;
                }

                if param_type.eq(&"@ContractState".to_string()) {
                    add_self = false;
                }
            }
        };

        (add_self, has_ref_self)
    }

    /// Check if the function has multiple IWorldDispatcher parameters.
    ///
    /// Returns
    ///  * a boolean indicating if the function has multiple world dispatchers.
    pub fn check_world_dispatcher(
        &mut self,
        db: &dyn SyntaxGroup,
        param_list: ast::ParamList,
    ) -> bool {
        let mut count = 0;

        param_list.elements(db).iter().for_each(|param| {
            let (_, _, param_type) = self.get_parameter_info(db, param.clone());

            if param_type.eq(&"IWorldDispatcher".to_string()) {
                count += 1;
            }
        });

        count > 1
    }

    /// Rewrites parameter list by:
    ///  * adding `self` parameter if missing,
    ///  * removing `world` if present as first parameter (self excluded), as it will be read from
    ///    the first function statement.
    ///
    /// Reports an error in case of:
    ///  * `ref self`, as systems are supposed to be 100% stateless,
    ///  * multiple IWorldDispatcher parameters.
    ///  * the `IWorldDispatcher` is not the first parameter (self excluded) and named 'world'.
    ///
    /// Returns
    ///  * the list of parameters in a String
    ///  * a boolean indicating if `self` has been added
    //   * a boolean indicating if `world` parameter has been removed
    pub fn rewrite_parameters(
        &mut self,
        db: &dyn SyntaxGroup,
        param_list: ast::ParamList,
        diagnostic_item: ids::SyntaxStablePtrId,
    ) -> (String, bool, bool) {
        let (add_self, has_ref_self) = self.check_self_parameter(db, param_list.clone());
        let has_multiple_world_dispatchers = self.check_world_dispatcher(db, param_list.clone());

        let mut world_removed = false;

        let mut params = param_list
            .elements(db)
            .iter()
            .enumerate()
            .filter_map(|(idx, param)| {
                let (name, modifiers, param_type) = self.get_parameter_info(db, param.clone());

                if param_type.eq(&"IWorldDispatcher".to_string())
                    && modifiers.eq(&"".to_string())
                    && !has_multiple_world_dispatchers
                {
                    let has_good_pos = (add_self && idx == 0) || (!add_self && idx == 1);
                    let has_good_name = name.eq(&"world".to_string());

                    if has_good_pos && has_good_name {
                        world_removed = true;
                        None
                    } else {
                        if !has_good_pos {
                            self.diagnostics.push(PluginDiagnostic {
                                stable_ptr: param.stable_ptr().untyped(),
                                message: "The IWorldDispatcher parameter must be the first \
                                          parameter of the function (self excluded)."
                                    .to_string(),
                                severity: Severity::Error,
                            });
                        }

                        if !has_good_name {
                            self.diagnostics.push(PluginDiagnostic {
                                stable_ptr: param.stable_ptr().untyped(),
                                message: "The IWorldDispatcher parameter must be named 'world'."
                                    .to_string(),
                                severity: Severity::Error,
                            });
                        }
                        Some(param.as_syntax_node().get_text(db))
                    }
                } else {
                    Some(param.as_syntax_node().get_text(db))
                }
            })
            .collect::<Vec<_>>();

        if has_multiple_world_dispatchers {
            self.diagnostics.push(PluginDiagnostic {
                stable_ptr: diagnostic_item,
                message: "Only one parameter of type IWorldDispatcher is allowed.".to_string(),
                severity: Severity::Error,
            });
        }

        if has_ref_self {
            self.diagnostics.push(PluginDiagnostic {
                stable_ptr: diagnostic_item,
                message: "Functions of dojo::contract cannot have 'ref self' parameter."
                    .to_string(),
                severity: Severity::Error,
            });
        }

        if add_self {
            params.insert(0, "self: @ContractState".to_string());
        }

        (params.join(", "), add_self, world_removed)
    }

    /// Rewrites function statements by adding the reading of `world` at first statement.
    pub fn rewrite_statements(
        &mut self,
        db: &dyn SyntaxGroup,
        statement_list: ast::StatementList,
    ) -> String {
        let mut statements = statement_list
            .elements(db)
            .iter()
            .map(|e| e.as_syntax_node().get_text(db))
            .collect::<Vec<_>>();

        statements.insert(0, "let world = self.world_dispatcher.read();\n".to_string());
        statements.join("")
    }

    /// Rewrites function declaration by:
    ///  * adding `self` parameter if missing,
    ///  * removing `world` if present as first parameter (self excluded),
    ///  * adding `let world = self.world_dispatcher.read();` statement at the beginning of the
    ///    function to restore the removed `world` parameter.
    pub fn rewrite_function(
        &mut self,
        db: &dyn SyntaxGroup,
        fn_ast: ast::FunctionWithBody,
    ) -> Vec<RewriteNode> {
        let mut rewritten_fn = RewriteNode::from_ast(&fn_ast);

        let (params_str, self_added, world_removed) = self.rewrite_parameters(
            db,
            fn_ast.declaration(db).signature(db).parameters(db),
            fn_ast.stable_ptr().untyped(),
        );

        if self_added || world_removed {
            let rewritten_params = rewritten_fn
                .modify_child(db, ast::FunctionWithBody::INDEX_DECLARATION)
                .modify_child(db, ast::FunctionDeclaration::INDEX_SIGNATURE)
                .modify_child(db, ast::FunctionSignature::INDEX_PARAMETERS);
            rewritten_params.set_str(params_str);
        }

        if world_removed {
            let rewritten_statements = rewritten_fn
                .modify_child(db, ast::FunctionWithBody::INDEX_BODY)
                .modify_child(db, ast::ExprBlock::INDEX_STATEMENTS);

            rewritten_statements
                .set_str(self.rewrite_statements(db, fn_ast.body(db).statements(db)));
        }

        vec![rewritten_fn]
    }

    /// Rewrites all the functions of a Impl block.
    fn rewrite_impl(&mut self, db: &dyn SyntaxGroup, impl_ast: ast::ItemImpl) -> Vec<RewriteNode> {
        if let ast::MaybeImplBody::Some(body) = impl_ast.body(db) {
            let body_nodes: Vec<_> = body
                .items(db)
                .elements(db)
                .iter()
                .flat_map(|el| {
                    if let ast::ImplItem::Function(fn_ast) = el {
                        return self.rewrite_function(db, fn_ast.clone());
                    }
                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            let mut builder = PatchBuilder::new(db);
            builder.add_modified(RewriteNode::interpolate_patched(
                "$body$",
                &UnorderedHashMap::from([(
                    "body".to_string(),
                    RewriteNode::new_modified(body_nodes),
                )]),
            ));

            let mut rewritten_impl = RewriteNode::from_ast(&impl_ast);
            let rewritten_items = rewritten_impl
                .modify_child(db, ast::ItemImpl::INDEX_BODY)
                .modify_child(db, ast::ImplBody::INDEX_ITEMS);

            rewritten_items.set_str(builder.code);
            return vec![rewritten_impl];
        }

        vec![RewriteNode::Copied(impl_ast.as_syntax_node())]
    }
}
