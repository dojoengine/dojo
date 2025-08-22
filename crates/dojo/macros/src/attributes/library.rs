use cairo_lang_macro::{Diagnostic, ProcMacroResult, TokenStream, quote};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{self, MaybeModuleBody};
use cairo_lang_syntax::node::with_db::SyntaxNodeWithDb;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use crate::constants::{CONSTRUCTOR_FN, DOJO_INIT_FN};
use crate::helpers::{DojoChecker, DojoParser, DojoTokenizer, ProcMacroResultExt};

#[derive(Debug)]
pub struct DojoLibrary {
    diagnostics: Vec<Diagnostic>,
    has_event: bool,
    has_storage: bool,
    has_init: bool,
    has_constructor: bool,
}

impl DojoLibrary {
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
            return DojoLibrary::process_ast(&db, &module_ast);
        }

        ProcMacroResult::fail("'dojo::library' must be used on module only.".to_string())
    }

    fn process_ast(db: &SimpleParserDatabase, module_ast: &ast::ItemModule) -> ProcMacroResult {
        let mut library = DojoLibrary::new();

        let name = module_ast.name(db).text(db).to_string();

        if let Some(failure) = DojoChecker::is_name_valid("library", &name) {
            return failure;
        }

        if let MaybeModuleBody::Some(body) = module_ast.body(db) {
            let mut body_nodes: Vec<_> = body
                .items(db)
                .elements(db)
                .map(|el| {
                    match el {
                        ast::ModuleItem::Enum(ref enum_ast) => {
                            if enum_ast.name(db).text(db) == "Event" {
                                return library.merge_event(db, enum_ast.clone());
                            }
                        }
                        ast::ModuleItem::Struct(ref struct_ast) => {
                            if struct_ast.name(db).text(db) == "Storage" {
                                return library.merge_storage(db, struct_ast.clone());
                            }
                        }
                        ast::ModuleItem::FreeFunction(ref fn_ast) => {
                            let fn_name = fn_ast.declaration(db).name(db).text(db);

                            if fn_name == CONSTRUCTOR_FN {
                                library.has_constructor = true;
                            }

                            if fn_name == DOJO_INIT_FN {
                                library.has_init = true;
                            }
                        }
                        _ => {}
                    };

                    let el = el.as_syntax_node();
                    let el = SyntaxNodeWithDb::new(&el, db);
                    quote! { #el }
                })
                .collect::<Vec<TokenStream>>();

            if library.has_constructor {
                return ProcMacroResult::fail(format!(
                    "The library {name} cannot have a constructor"
                ));
            }

            if library.has_init {
                return ProcMacroResult::fail(format!(
                    "The library {name} cannot have a {DOJO_INIT_FN}"
                ));
            }

            if !library.has_event {
                body_nodes.push(library.create_event())
            }

            if !library.has_storage {
                body_nodes.push(library.create_storage())
            }

            let library_code = DojoLibrary::generate_library_code(&name, body_nodes);
            return ProcMacroResult::finalize(library_code, library.diagnostics);
        }

        ProcMacroResult::fail(format!("The library '{name}' is empty."))
    }

    fn generate_library_code(name: &String, body: Vec<TokenStream>) -> TokenStream {
        let library_impl_name = DojoTokenizer::tokenize(&format!("{name}__LibraryImpl"));
        let dojo_name = DojoTokenizer::tokenize(&format!("\"{name}\""));
        let name = DojoTokenizer::tokenize(name);

        let mut content = TokenStream::new(vec![]);
        content.extend(body);

        quote! {
            #[starknet::contract]
            pub mod #name {
                use dojo::contract::components::world_provider::{world_provider_cpt, IWorldProvider};
                use dojo::contract::ILibrary;
                use dojo::meta::IDeployedResource;

                component!(path: world_provider_cpt, storage: world_provider, event: WorldProviderEvent);

                #[abi(embed_v0)]
                impl WorldProviderImpl = world_provider_cpt::WorldProviderImpl<ContractState>;

                #[abi(embed_v0)]
                pub impl #library_impl_name of ILibrary<ContractState> {}

                #[abi(embed_v0)]
                pub impl DojoDeployedLibraryImpl of IDeployedResource<ContractState> {
                    fn dojo_name(self: @ContractState) -> ByteArray {
                        #dojo_name
                    }
                }

                #[generate_trait]
                impl DojoLibraryInternalImpl of DojoLibraryInternalTrait {
                    fn world(self: @ContractState, namespace: @ByteArray) -> dojo::world::storage::WorldStorage {
                        dojo::world::WorldStorageTrait::new(self.world_provider.world_dispatcher(), namespace)
                    }
                }

                #content
            }
        }
    }

    pub fn merge_event(
        &mut self,
        db: &SimpleParserDatabase,
        enum_ast: ast::ItemEnum,
    ) -> TokenStream {
        self.has_event = true;

        let variants = enum_ast.variants(db).as_syntax_node();
        let variants = SyntaxNodeWithDb::new(&variants, db);

        quote! {
            #[event]
            #[derive(Drop, starknet::Event)]
            enum Event {
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
                #[flat]
                WorldProviderEvent: world_provider_cpt::Event,
            }
        }
    }

    pub fn merge_storage(
        &mut self,
        db: &SimpleParserDatabase,
        struct_ast: ast::ItemStruct,
    ) -> TokenStream {
        self.has_storage = true;

        let members = struct_ast.members(db).as_syntax_node();
        let members = SyntaxNodeWithDb::new(&members, db);

        quote! {
            #[storage]
            struct Storage {
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
                world_provider: world_provider_cpt::Storage,
            }
        }
    }
}
