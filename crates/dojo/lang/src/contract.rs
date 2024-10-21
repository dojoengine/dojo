use std::collections::HashMap;

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, MacroPluginMetadata, PluginDiagnostic, PluginGeneratedFile,
    PluginResult,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_plugins::plugins::HasItemsInCfgEx;
use cairo_lang_syntax::node::ast::{
    ArgClause, Expr, MaybeModuleBody, OptionArgListParenthesized, OptionReturnTypeClause,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, ids, Terminal, TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::system::Dependency;
use dojo_world::config::NamespaceConfig;
use dojo_world::contracts::naming;

use crate::plugin::{ContractAuxData, DojoAuxData, DOJO_CONTRACT_ATTR};
use crate::syntax::world_param::{self, WorldParamInjectionKind};
use crate::syntax::{self_param, utils as syntax_utils};

const DOJO_INIT_FN: &str = "dojo_init";
const CONTRACT_NAMESPACE: &str = "namespace";
const CONTRACT_NOMAPPING: &str = "nomapping";

#[derive(Debug, Clone, Default)]
pub struct ContractParameters {
    namespace: Option<String>,
    nomapping: bool,
}

#[derive(Debug)]
pub struct DojoContract {
    diagnostics: Vec<PluginDiagnostic>,
    dependencies: HashMap<smol_str::SmolStr, Dependency>,
    systems: Vec<String>,
}

impl DojoContract {
    pub fn from_module(
        db: &dyn SyntaxGroup,
        module_ast: &ast::ItemModule,
        namespace_config: &NamespaceConfig,
        metadata: &MacroPluginMetadata<'_>,
    ) -> PluginResult {
        let name = module_ast.name(db).text(db);

        let mut diagnostics = vec![];
        let parameters = get_parameters(db, module_ast, &mut diagnostics);

        let mut contract =
            DojoContract { diagnostics, dependencies: HashMap::new(), systems: vec![] };

        let mut has_event = false;
        let mut has_storage = false;
        let mut has_init = false;

        let unmapped_namespace = parameters.namespace.unwrap_or(namespace_config.default.clone());

        let contract_namespace = if parameters.nomapping {
            unmapped_namespace
        } else {
            // Maps namespace from the tag to ensure higher precision on matching namespace
            // mappings.
            namespace_config.get_mapping(&naming::get_tag(&unmapped_namespace, &name))
        };

        for (id, value) in [("name", &name.to_string()), ("namespace", &contract_namespace)] {
            if !NamespaceConfig::is_name_valid(value) {
                return PluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: module_ast.stable_ptr().0,
                        message: format!(
                            "The contract {id} '{value}' can only contain characters (a-z/A-Z), \
                             digits (0-9) and underscore (_)."
                        ),
                        severity: Severity::Error,
                    }],
                    remove_original_item: false,
                };
            }
        }

        let contract_tag = naming::get_tag(&contract_namespace, &name);
        let contract_name_hash = naming::compute_bytearray_hash(&name);
        let contract_namespace_hash = naming::compute_bytearray_hash(&contract_namespace);
        let contract_selector =
            naming::compute_selector_from_hashes(contract_namespace_hash, contract_name_hash);

        if let MaybeModuleBody::Some(body) = module_ast.body(db) {
            let mut body_nodes: Vec<_> = body
                .iter_items_in_cfg(db, metadata.cfg_set)
                .flat_map(|el| {
                    if let ast::ModuleItem::Enum(ref enum_ast) = el {
                        if enum_ast.name(db).text(db).to_string() == "Event" {
                            has_event = true;
                            return contract.merge_event(db, enum_ast.clone());
                        }
                    } else if let ast::ModuleItem::Struct(ref struct_ast) = el {
                        if struct_ast.name(db).text(db).to_string() == "Storage" {
                            has_storage = true;
                            return contract.merge_storage(db, struct_ast.clone());
                        }
                    } else if let ast::ModuleItem::Impl(ref impl_ast) = el {
                        // If an implementation is not targetting the ContractState,
                        // the auto injection of self and world is not applied.
                        let trait_path = impl_ast.trait_path(db).node.get_text(db);
                        if trait_path.contains("<ContractState>") {
                            return contract.rewrite_impl(db, impl_ast.clone(), metadata);
                        }
                    } else if let ast::ModuleItem::FreeFunction(ref fn_ast) = el {
                        let fn_decl = fn_ast.declaration(db);
                        let fn_name = fn_decl.name(db).text(db);

                        if fn_name == DOJO_INIT_FN {
                            has_init = true;
                            return contract.handle_init_fn(db, fn_ast);
                        }
                    }

                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            if !has_init {
                let node = RewriteNode::interpolate_patched(
                    "
                    #[starknet::interface]
                    pub trait IDojoInit<ContractState> {
                        fn $init_name$(self: @ContractState);
                    }

                    #[abi(embed_v0)]
                    pub impl IDojoInitImpl of IDojoInit<ContractState> {
                        fn $init_name$(self: @ContractState) {
                            if starknet::get_caller_address() != self.world().contract_address {
                                core::panics::panic_with_byte_array(
                                    @format!(\"Only the world can init contract `{}`, but caller \
                     is `{:?}`\",
                                    self.tag(),
                                    starknet::get_caller_address(),
                                ));
                            }
                        }
                    }
                ",
                    &UnorderedHashMap::from([(
                        "init_name".to_string(),
                        RewriteNode::Text(DOJO_INIT_FN.to_string()),
                    )]),
                );
                body_nodes.append(&mut vec![node]);
            }

            if !has_event {
                body_nodes.append(&mut contract.create_event())
            }

            if !has_storage {
                body_nodes.append(&mut contract.create_storage())
            }

            let mut builder = PatchBuilder::new(db, module_ast);
            builder.add_modified(RewriteNode::Mapped {
                node: Box::new(RewriteNode::interpolate_patched(
                    "
                #[starknet::contract]
                pub mod $name$ {
                    use dojo::world;
                    use dojo::world::IWorldDispatcher;
                    use dojo::world::IWorldDispatcherTrait;
                    use dojo::world::IWorldProvider;
                    use dojo::contract::IContract;
                    use starknet::storage::{
                        StorageMapReadAccess, StorageMapWriteAccess, StoragePointerReadAccess, \
                     StoragePointerWriteAccess
                    };

                    component!(path: dojo::contract::upgradeable::upgradeable, storage: \
                     upgradeable, event: UpgradeableEvent);

                    #[abi(embed_v0)]
                    pub impl ContractImpl of IContract<ContractState> {
                        fn name(self: @ContractState) -> ByteArray {
                            \"$name$\"
                        }

                        fn namespace(self: @ContractState) -> ByteArray {
                            \"$contract_namespace$\"
                        }

                        fn tag(self: @ContractState) -> ByteArray {
                            \"$contract_tag$\"
                        }

                        fn name_hash(self: @ContractState) -> felt252 {
                            $contract_name_hash$
                        }

                        fn namespace_hash(self: @ContractState) -> felt252 {
                            $contract_namespace_hash$
                        }

                        fn selector(self: @ContractState) -> felt252 {
                            $contract_selector$
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
                     dojo::contract::upgradeable::upgradeable::UpgradableImpl<ContractState>;

                    $body$
                }
                ",
                    &UnorderedHashMap::from([
                        ("name".to_string(), RewriteNode::Text(name.to_string())),
                        ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                        (
                            "contract_namespace".to_string(),
                            RewriteNode::Text(contract_namespace.clone()),
                        ),
                        (
                            "contract_name_hash".to_string(),
                            RewriteNode::Text(contract_name_hash.to_string()),
                        ),
                        (
                            "contract_namespace_hash".to_string(),
                            RewriteNode::Text(contract_namespace_hash.to_string()),
                        ),
                        (
                            "contract_selector".to_string(),
                            RewriteNode::Text(contract_selector.to_string()),
                        ),
                        ("contract_tag".to_string(), RewriteNode::Text(contract_tag)),
                    ]),
                )),
                origin: module_ast.as_syntax_node().span_without_trivia(db),
            });

            let (code, code_mappings) = builder.build();

            return PluginResult {
                code: Some(PluginGeneratedFile {
                    name: name.clone(),
                    content: code,
                    aux_data: Some(DynGeneratedFileAuxData::new(DojoAuxData {
                        models: vec![],
                        contracts: vec![ContractAuxData {
                            name,
                            namespace: contract_namespace.clone(),
                            dependencies: contract.dependencies.values().cloned().collect(),
                            systems: contract.systems.clone(),
                        }],
                        events: vec![],
                    })),
                    code_mappings,
                }),
                diagnostics: contract.diagnostics,
                remove_original_item: true,
            };
        }

        PluginResult::default()
    }

    fn handle_init_fn(
        &mut self,
        db: &dyn SyntaxGroup,
        fn_ast: &ast::FunctionWithBody,
    ) -> Vec<RewriteNode> {
        let fn_decl = fn_ast.declaration(db);

        if let OptionReturnTypeClause::ReturnTypeClause(_) = fn_decl.signature(db).ret_ty(db) {
            self.diagnostics.push(PluginDiagnostic {
                stable_ptr: fn_ast.stable_ptr().untyped(),
                message: "The dojo_init function cannot have a return type.".to_string(),
                severity: Severity::Error,
            });
        }

        let (params_str, was_world_injected) = self.rewrite_parameters(
            db,
            fn_decl.signature(db).parameters(db),
            fn_ast.stable_ptr().untyped(),
        );

        let trait_node = RewriteNode::interpolate_patched(
            "#[starknet::interface]
            pub trait IDojoInit<ContractState> {
                fn $init_name$($params_str$);
            }
            ",
            &UnorderedHashMap::from([
                ("init_name".to_string(), RewriteNode::Text(DOJO_INIT_FN.to_string())),
                ("params_str".to_string(), RewriteNode::Text(params_str.clone())),
            ]),
        );

        let impl_node = RewriteNode::Text(
            "
            #[abi(embed_v0)]
            pub impl IDojoInitImpl of IDojoInit<ContractState> {
            "
            .to_string(),
        );

        let declaration_node = RewriteNode::Mapped {
            node: Box::new(RewriteNode::Text(format!("fn {}({}) {{", DOJO_INIT_FN, params_str))),
            origin: fn_ast.declaration(db).as_syntax_node().span_without_trivia(db),
        };

        let world_line_node = if was_world_injected {
            RewriteNode::Text("let world = self.world_dispatcher.read();".to_string())
        } else {
            RewriteNode::empty()
        };

        let assert_world_caller_node = RewriteNode::Text(
            "if starknet::get_caller_address() != self.world().contract_address { \
             core::panics::panic_with_byte_array(@format!(\"Only the world can init contract \
             `{}`, but caller is `{:?}`\", self.tag(), starknet::get_caller_address())); }"
                .to_string(),
        );

        let func_nodes = fn_ast
            .body(db)
            .statements(db)
            .elements(db)
            .iter()
            .map(|e| RewriteNode::Mapped {
                node: Box::new(RewriteNode::from(e.as_syntax_node())),
                origin: e.as_syntax_node().span_without_trivia(db),
            })
            .collect::<Vec<_>>();

        let mut nodes = vec![
            trait_node,
            impl_node,
            declaration_node,
            world_line_node,
            assert_world_caller_node,
        ];
        nodes.extend(func_nodes);
        // Close the init function + close the impl block.
        nodes.push(RewriteNode::Text("}\n}".to_string()));

        nodes
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
                UpgradeableEvent: dojo::contract::upgradeable::upgradeable::Event,
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
                UpgradeableEvent: dojo::contract::upgradeable::upgradeable::Event,
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
                upgradeable: dojo::contract::upgradeable::upgradeable::Storage,
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
                upgradeable: dojo::contract::upgradeable::upgradeable::Storage,
            }
            "
            .to_string(),
        )]
    }

    /// Rewrites parameter list by:
    ///  * adding `self` parameter based on the `world` parameter mutability. If `world` is not
    ///    provided, a `View` is assumed.
    ///  * removing `world` if present as first parameter, as it will be read from the first
    ///    function statement.
    ///
    /// Reports an error in case of:
    ///  * `self` used explicitly,
    ///  * multiple world parameters,
    ///  * the `world` parameter is not the first parameter and named 'world'.
    ///
    /// Returns
    ///  * the list of parameters in a String.
    ///  * true if the world has to be injected (found as the first param).
    pub fn rewrite_parameters(
        &mut self,
        db: &dyn SyntaxGroup,
        param_list: ast::ParamList,
        fn_diagnostic_item: ids::SyntaxStablePtrId,
    ) -> (String, bool) {
        let is_self_used = self_param::check_parameter(db, &param_list);

        let world_injection = world_param::parse_world_injection(
            db,
            param_list.clone(),
            fn_diagnostic_item,
            &mut self.diagnostics,
        );

        if is_self_used && world_injection != WorldParamInjectionKind::None {
            self.diagnostics.push(PluginDiagnostic {
                stable_ptr: fn_diagnostic_item,
                message: "You cannot use `self` and `world` parameters together.".to_string(),
                severity: Severity::Error,
            });
        }

        let mut params = param_list
            .elements(db)
            .iter()
            .filter_map(|param| {
                let (name, _, param_type) = syntax_utils::get_parameter_info(db, param.clone());

                // If the param is `IWorldDispatcher`, we don't need to keep it in the param list
                // as it is flatten in the first statement.
                if world_param::is_world_param(&name, &param_type) {
                    None
                } else {
                    Some(param.as_syntax_node().get_text(db))
                }
            })
            .collect::<Vec<_>>();

        match world_injection {
            WorldParamInjectionKind::None => {
                if !is_self_used {
                    params.insert(0, "self: @ContractState".to_string());
                }
            }
            WorldParamInjectionKind::View => {
                params.insert(0, "self: @ContractState".to_string());
            }
            WorldParamInjectionKind::External => {
                params.insert(0, "ref self: ContractState".to_string());
            }
        }

        (params.join(", "), world_injection != WorldParamInjectionKind::None)
    }

    /// Rewrites function declaration by:
    ///  * adding `self` parameter if missing,
    ///  * removing `world` if present as first parameter (self excluded),
    ///  * adding `let world = self.world_dispatcher.read();` statement at the beginning of the
    ///    function to restore the removed `world` parameter.
    ///  * if `has_generate_trait` is true, the implementation containing the function has the
    ///    #[generate_trait] attribute.
    pub fn rewrite_function(
        &mut self,
        db: &dyn SyntaxGroup,
        fn_ast: ast::FunctionWithBody,
        has_generate_trait: bool,
    ) -> Vec<RewriteNode> {
        let fn_name = fn_ast.declaration(db).name(db).text(db);
        let return_type =
            fn_ast.declaration(db).signature(db).ret_ty(db).as_syntax_node().get_text(db);

        // Consider the function as a system if no return type is specified.
        if return_type.is_empty() {
            self.systems.push(fn_name.to_string());
        }

        let (params_str, was_world_injected) = self.rewrite_parameters(
            db,
            fn_ast.declaration(db).signature(db).parameters(db),
            fn_ast.stable_ptr().untyped(),
        );

        let declaration_node = RewriteNode::Mapped {
            node: Box::new(RewriteNode::Text(format!(
                "fn {}({}) {} {{",
                fn_name, params_str, return_type
            ))),
            origin: fn_ast.declaration(db).as_syntax_node().span_without_trivia(db),
        };

        let world_line_node = if was_world_injected {
            RewriteNode::Text("let world = self.world_dispatcher.read();".to_string())
        } else {
            RewriteNode::empty()
        };

        let func_nodes = fn_ast
            .body(db)
            .statements(db)
            .elements(db)
            .iter()
            .map(|e| RewriteNode::Mapped {
                node: Box::new(RewriteNode::from(e.as_syntax_node())),
                origin: e.as_syntax_node().span_without_trivia(db),
            })
            .collect::<Vec<_>>();

        if has_generate_trait && was_world_injected {
            self.diagnostics.push(PluginDiagnostic {
                stable_ptr: fn_ast.stable_ptr().untyped(),
                message: "You cannot use `world` and `#[generate_trait]` together. Use `self` \
                          instead."
                    .to_string(),
                severity: Severity::Error,
            });
        }

        let mut nodes = vec![declaration_node, world_line_node];
        nodes.extend(func_nodes);
        nodes.push(RewriteNode::Text("}".to_string()));

        nodes
    }

    /// Rewrites all the functions of a Impl block.
    fn rewrite_impl(
        &mut self,
        db: &dyn SyntaxGroup,
        impl_ast: ast::ItemImpl,
        metadata: &MacroPluginMetadata<'_>,
    ) -> Vec<RewriteNode> {
        let generate_attrs = impl_ast.attributes(db).query_attr(db, "generate_trait");
        let has_generate_trait = !generate_attrs.is_empty();

        if let ast::MaybeImplBody::Some(body) = impl_ast.body(db) {
            // We shouldn't have generic param in the case of contract's endpoints.
            let impl_node = RewriteNode::Mapped {
                node: Box::new(RewriteNode::Text(format!(
                    "{} impl {} of {} {{",
                    impl_ast.attributes(db).as_syntax_node().get_text(db),
                    impl_ast.name(db).as_syntax_node().get_text(db),
                    impl_ast.trait_path(db).as_syntax_node().get_text(db),
                ))),
                origin: impl_ast.as_syntax_node().span_without_trivia(db),
            };

            let body_nodes: Vec<_> = body
                .iter_items_in_cfg(db, metadata.cfg_set)
                .flat_map(|el| {
                    if let ast::ImplItem::Function(ref fn_ast) = el {
                        return self.rewrite_function(db, fn_ast.clone(), has_generate_trait);
                    }
                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            let body_node = RewriteNode::Mapped {
                node: Box::new(RewriteNode::interpolate_patched(
                    "$body$",
                    &UnorderedHashMap::from([(
                        "body".to_string(),
                        RewriteNode::new_modified(body_nodes),
                    )]),
                )),
                origin: impl_ast.as_syntax_node().span_without_trivia(db),
            };

            return vec![impl_node, body_node, RewriteNode::Text("}".to_string())];
        }

        vec![RewriteNode::Copied(impl_ast.as_syntax_node())]
    }
}

/// Get the contract namespace from the `Expr` parameter.
fn get_contract_namespace(
    db: &dyn SyntaxGroup,
    arg_value: Expr,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> Option<String> {
    match arg_value {
        Expr::ShortString(ss) => Some(ss.string_value(db).unwrap()),
        Expr::String(s) => Some(s.string_value(db).unwrap()),
        _ => {
            diagnostics.push(PluginDiagnostic {
                message: format!(
                    "The argument '{}' of dojo::contract must be a string",
                    CONTRACT_NAMESPACE
                ),
                stable_ptr: arg_value.stable_ptr().untyped(),
                severity: Severity::Error,
            });
            Option::None
        }
    }
}

/// Get parameters of the dojo::contract attribute.
///
/// Parameters:
/// * db: The semantic database.
/// * module_ast: The AST of the contract module.
/// * diagnostics: vector of compiler diagnostics.
///
/// Returns:
/// * A [`ContractParameters`] object containing all the dojo::contract parameters with their
///   default values if not set in the code.
fn get_parameters(
    db: &dyn SyntaxGroup,
    module_ast: &ast::ItemModule,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> ContractParameters {
    let mut parameters = ContractParameters::default();
    let mut processed_args: HashMap<String, bool> = HashMap::new();

    if let OptionArgListParenthesized::ArgListParenthesized(arguments) =
        module_ast.attributes(db).query_attr(db, DOJO_CONTRACT_ATTR).first().unwrap().arguments(db)
    {
        arguments.arguments(db).elements(db).iter().for_each(|a| match a.arg_clause(db) {
            ArgClause::Named(x) => {
                let arg_name = x.name(db).text(db).to_string();
                let arg_value = x.value(db);

                if processed_args.contains_key(&arg_name) {
                    diagnostics.push(PluginDiagnostic {
                        message: format!("Too many '{}' attributes for dojo::contract", arg_name),
                        stable_ptr: module_ast.stable_ptr().untyped(),
                        severity: Severity::Error,
                    });
                } else {
                    processed_args.insert(arg_name.clone(), true);

                    match arg_name.as_str() {
                        CONTRACT_NAMESPACE => {
                            parameters.namespace =
                                get_contract_namespace(db, arg_value, diagnostics);
                        }
                        CONTRACT_NOMAPPING => {
                            parameters.nomapping = true;
                        }
                        _ => {
                            diagnostics.push(PluginDiagnostic {
                                message: format!(
                                    "Unexpected argument '{}' for dojo::contract",
                                    arg_name
                                ),
                                stable_ptr: x.stable_ptr().untyped(),
                                severity: Severity::Warning,
                            });
                        }
                    }
                }
            }
            ArgClause::Unnamed(arg) => {
                let arg_name = arg.value(db).as_syntax_node().get_text(db);

                diagnostics.push(PluginDiagnostic {
                    message: format!("Unexpected argument '{}' for dojo::contract", arg_name),
                    stable_ptr: arg.stable_ptr().untyped(),
                    severity: Severity::Warning,
                });
            }
            ArgClause::FieldInitShorthand(x) => {
                diagnostics.push(PluginDiagnostic {
                    message: format!(
                        "Unexpected argument '{}' for dojo::contract",
                        x.name(db).name(db).text(db).to_string()
                    ),
                    stable_ptr: x.stable_ptr().untyped(),
                    severity: Severity::Warning,
                });
            }
        })
    }

    parameters
}
