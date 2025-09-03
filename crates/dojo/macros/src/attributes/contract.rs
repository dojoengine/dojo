use cairo_lang_macro::{Diagnostic, ProcMacroResult, TokenStream, quote};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{self, MaybeModuleBody, OptionReturnTypeClause};
use cairo_lang_syntax::node::with_db::SyntaxNodeWithDb;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use crate::constants::{CONSTRUCTOR_FN, DOJO_INIT_FN};
use crate::helpers::{DiagnosticsExt, DojoChecker, DojoParser, DojoTokenizer, ProcMacroResultExt};

#[derive(Debug)]
pub struct DojoContract {
    diagnostics: Vec<Diagnostic>,
    has_event: bool,
    has_storage: bool,
    has_init: bool,
    has_constructor: bool,
}

impl DojoContract {
    pub fn new() -> Self {
        Self {
            diagnostics: vec![],
            has_event: false,
            has_storage: false,
            has_init: false,
            has_constructor: false,
        }
    }

    pub fn process(token_stream: TokenStream) -> ProcMacroResult {
        let db = SimpleParserDatabase::default();

        if let Some(module_ast) = DojoParser::parse_and_find_module(&db, &token_stream) {
            return DojoContract::process_ast(&db, &module_ast);
        }

        ProcMacroResult::fail("'dojo::contract' must be used on module only.".to_string())
    }

    fn process_ast(db: &SimpleParserDatabase, module_ast: &ast::ItemModule) -> ProcMacroResult {
        let mut contract = DojoContract::new();

        let name = module_ast.name(db).text(db).to_string();

        if let Some(failure) = DojoChecker::is_name_valid("contract", &name) {
            return failure;
        }

        if let MaybeModuleBody::Some(body) = module_ast.body(db) {
            let mut body_nodes = body
                .items(db)
                .elements(db)
                .map(|el| {
                    match el {
                        ast::ModuleItem::Enum(ref enum_ast) => {
                            if enum_ast.name(db).text(db) == "Event" {
                                return contract.merge_event(db, enum_ast);
                            }
                        }
                        ast::ModuleItem::Struct(ref struct_ast) => {
                            if struct_ast.name(db).text(db) == "Storage" {
                                return contract.merge_storage(db, struct_ast);
                            }
                        }
                        ast::ModuleItem::FreeFunction(ref fn_ast) => {
                            let fn_name = fn_ast.declaration(db).name(db).text(db);

                            if fn_name == CONSTRUCTOR_FN {
                                return contract.handle_constructor_fn(db, fn_ast);
                            }

                            if fn_name == DOJO_INIT_FN {
                                return contract.handle_init_fn(db, fn_ast);
                            }
                        }
                        _ => {}
                    };

                    let el = el.as_syntax_node();
                    let el = SyntaxNodeWithDb::new(&el, db);
                    quote! { #el }
                })
                .collect::<Vec<TokenStream>>();

            if !contract.has_constructor {
                body_nodes.push(contract.create_constructor());
            }

            if !contract.has_init {
                body_nodes.push(contract.create_init_fn());
            }

            if !contract.has_event {
                body_nodes.push(contract.create_event());
            }

            if !contract.has_storage {
                body_nodes.push(contract.create_storage());
            }

            let contract_code = DojoContract::generate_contract_code(&name, body_nodes);
            return ProcMacroResult::finalize(contract_code, contract.diagnostics);
        }

        ProcMacroResult::fail(format!("The contract '{name}' is empty."))
    }

    fn generate_contract_code(name: &String, body: Vec<TokenStream>) -> TokenStream {
        let contract_impl_name = DojoTokenizer::tokenize(&format!("{name}__ContractImpl"));
        let dojo_name = DojoTokenizer::tokenize(&format!("\"{name}\""));
        let name = DojoTokenizer::tokenize(name);

        let mut content = TokenStream::new(vec![]);
        content.extend(body);

        quote! {
            #[starknet::contract]
            pub mod #name {
                use dojo::contract::components::world_provider::{
                    world_provider_cpt, world_provider_cpt::InternalTrait as WorldProviderInternal, IWorldProvider
                };
                use dojo::contract::components::upgradeable::upgradeable_cpt;
                use dojo::contract::IContract;
                use dojo::meta::IDeployedResource;

                component!(path: world_provider_cpt, storage: world_provider, event: WorldProviderEvent);
                component!(path: upgradeable_cpt, storage: upgradeable, event: UpgradeableEvent);

                #[abi(embed_v0)]
                impl WorldProviderImpl = world_provider_cpt::WorldProviderImpl<ContractState>;

                #[abi(embed_v0)]
                impl UpgradeableImpl = upgradeable_cpt::UpgradeableImpl<ContractState>;

                #[abi(embed_v0)]
                pub impl #contract_impl_name of IContract<ContractState> {}

                #[abi(embed_v0)]
                pub impl DojoDeployedContractImpl of IDeployedResource<ContractState> {
                    fn dojo_name(self: @ContractState) -> ByteArray {
                        #dojo_name
                    }
                }

                #[generate_trait]
                impl DojoContractInternalImpl of DojoContractInternalTrait {
                    fn world(self: @ContractState, namespace: @ByteArray) -> dojo::world::storage::WorldStorage {
                        dojo::world::WorldStorageTrait::new(self.world_provider.world_dispatcher(), namespace)
                    }

                    fn world_ns_hash(self: @ContractState, namespace_hash: felt252) -> dojo::world::storage::WorldStorage {
                        dojo::world::WorldStorageTrait::new_from_hash(self.world_provider.world_dispatcher(), namespace_hash)
                    }
                }

                #content
            }
        }
    }

    /// If a constructor is provided, we should keep the user statements.
    /// We only inject the world provider initializer.
    fn handle_constructor_fn(
        &mut self,
        db: &SimpleParserDatabase,
        fn_ast: &ast::FunctionWithBody,
    ) -> TokenStream {
        self.has_constructor = true;

        if !is_valid_constructor_params(db, fn_ast) {
            self.diagnostics.push_error(format!(
                "The constructor must have exactly one parameter, which is `ref self: \
                 ContractState`. Add a `{DOJO_INIT_FN}` function instead if you need to \
                 initialize the contract with parameters."
            ));
        }

        let ctor_decl = fn_ast.declaration(db).as_syntax_node();
        let ctor_decl = SyntaxNodeWithDb::new(&ctor_decl, db);

        let ctor_body = fn_ast.body(db).as_syntax_node();
        let ctor_body = SyntaxNodeWithDb::new(&ctor_body, db);

        quote! {
            #[constructor]
            #ctor_decl {
                self.world_provider.initializer();
                #ctor_body
            }
        }
    }

    fn create_constructor(&self) -> TokenStream {
        quote! {
            #[constructor]
            fn constructor(ref self: ContractState) {
                self.world_provider.initializer();
            }
        }
    }

    fn handle_init_fn(
        &mut self,
        db: &SimpleParserDatabase,
        fn_ast: &ast::FunctionWithBody,
    ) -> TokenStream {
        self.has_init = true;

        if let OptionReturnTypeClause::ReturnTypeClause(_) =
            fn_ast.declaration(db).signature(db).ret_ty(db)
        {
            self.diagnostics
                .push_error(format!("The {DOJO_INIT_FN} function cannot have a return type."));
        }

        let fn_decl = fn_ast.declaration(db).as_syntax_node();
        let fn_decl = SyntaxNodeWithDb::new(&fn_decl, db);

        let fn_body = fn_ast.body(db).as_syntax_node();
        let fn_body = SyntaxNodeWithDb::new(&fn_body, db);

        quote! {
            #[abi(per_item)]
            #[generate_trait]
            pub impl IDojoInitImpl of IDojoInit {
                #[external(v0)]
                #fn_decl {
                    if starknet::get_caller_address() != self.world_provider.world_dispatcher().contract_address {
                        core::panics::panic_with_byte_array(
                            @format!(
                                "Only the world can init contract `{}`, but caller is `{:?}`",
                                self.dojo_name(),
                                starknet::get_caller_address()
                            )
                        );
                    }
                    #fn_body
                }
            }
        }
    }

    fn create_init_fn(&self) -> TokenStream {
        let init_name = DojoTokenizer::tokenize(DOJO_INIT_FN);

        quote! {
            #[abi(per_item)]
            #[generate_trait]
            pub impl IDojoInitImpl of IDojoInit {
                #[external(v0)]
                fn #init_name(self: @ContractState) {
                    if starknet::get_caller_address() != self.world_provider.world_dispatcher().contract_address {
                        core::panics::panic_with_byte_array(
                            @format!("Only the world can init contract `{}`, but caller is `{:?}`",
                            self.dojo_name(),
                            starknet::get_caller_address(),
                        ));
                    }
                }
            }
        }
    }

    pub fn merge_event(
        &mut self,
        db: &SimpleParserDatabase,
        enum_ast: &ast::ItemEnum,
    ) -> TokenStream {
        self.has_event = true;

        let variants = enum_ast.variants(db).as_syntax_node();
        let variants = SyntaxNodeWithDb::new(&variants, db);

        quote! {
            #[event]
            #[derive(Drop, starknet::Event)]
            enum Event {
                UpgradeableEvent: upgradeable_cpt::Event,
                WorldProviderEvent: world_provider_cpt::Event,
                #variants
            }
        }
    }

    pub fn create_event(&mut self) -> TokenStream {
        quote! {
            #[event]
            #[derive(Drop, starknet::Event)]
            enum Event {
                UpgradeableEvent: upgradeable_cpt::Event,
                WorldProviderEvent: world_provider_cpt::Event,
            }
        }
    }

    pub fn merge_storage(
        &mut self,
        db: &SimpleParserDatabase,
        struct_ast: &ast::ItemStruct,
    ) -> TokenStream {
        self.has_storage = true;

        let members = struct_ast.members(db).as_syntax_node();
        let members = SyntaxNodeWithDb::new(&members, db);

        quote! {
            #[storage]
            struct Storage {
                #[substorage(v0)]
                upgradeable: upgradeable_cpt::Storage,
                #[substorage(v0)]
                world_provider: world_provider_cpt::Storage,
                #members
            }
        }
    }

    pub fn create_storage(&mut self) -> TokenStream {
        quote! {
            #[storage]
            struct Storage {
                #[substorage(v0)]
                upgradeable: upgradeable_cpt::Storage,
                #[substorage(v0)]
                world_provider: world_provider_cpt::Storage,
            }
        }
    }
}

/// Checks if the constructor parameters are valid.
/// We only allow one parameter for the constructor, which is the contract state,
/// since `dojo_init` is called by the world after every resource has been deployed.
fn is_valid_constructor_params(db: &SimpleParserDatabase, fn_ast: &ast::FunctionWithBody) -> bool {
    let mut params = fn_ast.declaration(db).signature(db).parameters(db).elements(db);
    params.len() == 1
        && params.next().unwrap().as_syntax_node().get_text(db).contains("ref self: ContractState")
}

// TODO RBA/
// #[cfg(test)]
// mod tests {
// use super::*;
//
// #[test]
// fn test_is_valid_constructor_params_ok() {
// assert!(is_valid_constructor_params("ref self: ContractState"));
// assert!(is_valid_constructor_params("ref self: ContractState "));
// assert!(is_valid_constructor_params(" ref self: ContractState"));
// }
//
// #[test]
// fn test_is_valid_constructor_params_not_ok() {
// assert!(!is_valid_constructor_params(""));
// assert!(!is_valid_constructor_params("self: ContractState"));
// assert!(!is_valid_constructor_params("ref self: OtherState"));
// assert!(!is_valid_constructor_params("ref self: ContractState, other: felt252"));
// assert!(!is_valid_constructor_params("other: felt252"));
// }
// }
