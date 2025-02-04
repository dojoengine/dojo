use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, MacroPluginMetadata, PluginDiagnostic, PluginGeneratedFile,
    PluginResult,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_plugins::plugins::HasItemsInCfgEx;
use cairo_lang_syntax::node::ast::{MaybeModuleBody, OptionReturnTypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::naming;

use crate::aux_data::ContractAuxData;

const CONTRACT_PATCH: &str = include_str!("./patches/contract.patch.cairo");
const DEFAULT_INIT_PATCH: &str = include_str!("./patches/default_init.patch.cairo");
const CONSTRUCTOR_FN: &str = "constructor";
const DOJO_INIT_FN: &str = "dojo_init";

#[derive(Debug, Clone, Default)]
pub struct ContractParameters {
    pub namespace: Option<String>,
}

#[derive(Debug)]
pub struct DojoContract {
    diagnostics: Vec<PluginDiagnostic>,
    systems: Vec<String>,
}

impl DojoContract {
    pub fn from_module(
        db: &dyn SyntaxGroup,
        module_ast: &ast::ItemModule,
        metadata: &MacroPluginMetadata<'_>,
    ) -> PluginResult {
        let name = module_ast.name(db).text(db);

        let mut contract = DojoContract { diagnostics: vec![], systems: vec![] };

        for (id, value) in [("name", &name.to_string())] {
            if !naming::is_name_valid(value) {
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

        let mut has_event = false;
        let mut has_storage = false;
        let mut has_init = false;
        let mut has_constructor = false;

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
                    } else if let ast::ModuleItem::FreeFunction(ref fn_ast) = el {
                        let fn_decl = fn_ast.declaration(db);
                        let fn_name = fn_decl.name(db).text(db);

                        if fn_name == CONSTRUCTOR_FN {
                            has_constructor = true;
                            return contract.handle_constructor_fn(db, fn_ast);
                        }

                        if fn_name == DOJO_INIT_FN {
                            has_init = true;
                            return contract.handle_init_fn(db, fn_ast);
                        }
                    }

                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            if !has_constructor {
                let node = RewriteNode::Text(
                    "
                    #[constructor]
                        fn constructor(ref self: ContractState) {
                            self.world_provider.initializer();
                        }
                    "
                    .to_string(),
                );

                body_nodes.append(&mut vec![node]);
            }

            if !has_init {
                let node = RewriteNode::interpolate_patched(
                    DEFAULT_INIT_PATCH,
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
                    CONTRACT_PATCH,
                    &UnorderedHashMap::from([
                        ("name".to_string(), RewriteNode::Text(name.to_string())),
                        ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                    ]),
                )),
                origin: module_ast.as_syntax_node().span_without_trivia(db),
            });

            let (code, code_mappings) = builder.build();

            crate::debug_expand(&format!("CONTRACT PATCH: {name}"), &code);

            return PluginResult {
                code: Some(PluginGeneratedFile {
                    name: name.clone(),
                    content: code,
                    aux_data: Some(DynGeneratedFileAuxData::new(ContractAuxData {
                        name: name.to_string(),
                        systems: contract.systems.clone(),
                    })),
                    code_mappings,
                    diagnostics_note: None,
                }),
                diagnostics: contract.diagnostics,
                remove_original_item: true,
            };
        }

        PluginResult::default()
    }

    /// If a constructor is provided, we should keep the user statements.
    /// We only inject the world provider initializer.
    fn handle_constructor_fn(
        &mut self,
        db: &dyn SyntaxGroup,
        fn_ast: &ast::FunctionWithBody,
    ) -> Vec<RewriteNode> {
        let fn_decl = fn_ast.declaration(db);

        let params_str = self.params_to_str(db, fn_decl.signature(db).parameters(db));
        if !is_valid_constructor_params(&params_str) {
            self.diagnostics.push(PluginDiagnostic {
                stable_ptr: fn_ast.stable_ptr().untyped(),
                message: "The constructor must have exactly one parameter, which is `ref self: \
                          ContractState`. Add a `dojo_init` function instead if you need to \
                          initialize the contract with parameters."
                    .to_string(),
                severity: Severity::Error,
            });
        }

        let declaration_node = RewriteNode::Mapped {
            node: Box::new(RewriteNode::Text(format!(
                "
                #[constructor]
                fn constructor({}) {{
                    self.world_provider.initializer();
                ",
                params_str
            ))),
            origin: fn_ast.declaration(db).as_syntax_node().span_without_trivia(db),
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

        let mut nodes = vec![declaration_node];

        nodes.extend(func_nodes);

        // Close the constructor with users statements included.
        nodes.push(RewriteNode::Text("}\n".to_string()));

        nodes
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
                message: format!("The {} function cannot have a return type.", DOJO_INIT_FN)
                    .to_string(),
                severity: Severity::Error,
            });
        }

        let params: Vec<String> = fn_decl
            .signature(db)
            .parameters(db)
            .elements(db)
            .iter()
            .map(|p| p.as_syntax_node().get_text(db))
            .collect::<Vec<_>>();

        let params_str = params.join(", ");

        // Since the dojo init is meant to be called by the world, we don't need an
        // interface to be generated (which adds a considerable amount of code).
        let impl_node = RewriteNode::Text(
            "
            #[abi(per_item)]
            #[generate_trait]
            pub impl IDojoInitImpl of IDojoInit {
                #[external(v0)]
            "
            .to_string(),
        );

        let declaration_node = RewriteNode::Mapped {
            node: Box::new(RewriteNode::Text(format!("fn {}({}) {{", DOJO_INIT_FN, params_str))),
            origin: fn_ast.declaration(db).as_syntax_node().span_without_trivia(db),
        };

        // Asserts the caller is the world, and close the init function.
        let assert_world_caller_node = RewriteNode::Text(
            "if starknet::get_caller_address() != \
             self.world_provider.world_dispatcher().contract_address { \
             core::panics::panic_with_byte_array(@format!(\"Only the world can init contract \
             `{}`, but caller is `{:?}`\", self.dojo_name(), starknet::get_caller_address())); }"
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

        let mut nodes = vec![impl_node, declaration_node, assert_world_caller_node];
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
                UpgradeableEvent: upgradeable_cpt::Event,
                WorldProviderEvent: world_provider_cpt::Event,
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
                UpgradeableEvent: upgradeable_cpt::Event,
                WorldProviderEvent: world_provider_cpt::Event,
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
                #[substorage(v0)]
                upgradeable: upgradeable_cpt::Storage,
                #[substorage(v0)]
                world_provider: world_provider_cpt::Storage,
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
                #[substorage(v0)]
                upgradeable: upgradeable_cpt::Storage,
                #[substorage(v0)]
                world_provider: world_provider_cpt::Storage,
            }
            "
            .to_string(),
        )]
    }

    /// Converts parameter list to it's string representation.
    pub fn params_to_str(&mut self, db: &dyn SyntaxGroup, param_list: ast::ParamList) -> String {
        let params = param_list
            .elements(db)
            .iter()
            .map(|param| param.as_syntax_node().get_text(db))
            .collect::<Vec<_>>();

        params.join(", ")
    }
}

/// Checks if the constructor parameters are valid.
/// We only allow one parameter for the constructor, which is the contract state,
/// since `dojo_init` is called by the world after every resource has been deployed.
fn is_valid_constructor_params(params: &str) -> bool {
    let frags = params.split(",").collect::<Vec<_>>();

    if frags.len() != 1 {
        return false;
    }

    frags.first().unwrap().contains("ref self: ContractState")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_constructor_params_ok() {
        assert!(is_valid_constructor_params("ref self: ContractState"));
        assert!(is_valid_constructor_params("ref self: ContractState "));
        assert!(is_valid_constructor_params(" ref self: ContractState"));
    }

    #[test]
    fn test_is_valid_constructor_params_not_ok() {
        assert!(!is_valid_constructor_params(""));
        assert!(!is_valid_constructor_params("self: ContractState"));
        assert!(!is_valid_constructor_params("ref self: OtherState"));
        assert!(!is_valid_constructor_params("ref self: ContractState, other: felt252"));
        assert!(!is_valid_constructor_params("other: felt252"));
    }
}
