//! `dojo_contract` attribute macro.

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_macro::{attribute_macro, Diagnostic, Diagnostics, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{MaybeModuleBody, OptionReturnTypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::BodyItems;
use cairo_lang_syntax::node::kind::SyntaxKind::ItemModule;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

use super::constants::DOJO_CONTRACT_ATTR;
use super::struct_parser::{validate_attributes, validate_namings_diagnostics};
use crate::diagnostic_ext::DiagnosticsExt;

const CONSTRUCTOR_FN: &str = "constructor";
pub const DOJO_INIT_FN: &str = "dojo_init";

const CONTRACT_PATCH: &str = include_str!("./patches/contract.patch.cairo");
const DEFAULT_INIT_PATCH: &str = include_str!("./patches/default_init.patch.cairo");

#[attribute_macro("dojo::contract")]
pub fn dojo_contract(_args: TokenStream, token_stream: TokenStream) -> ProcMacroResult {
    handle_module_attribute_macro(token_stream)
}

pub fn handle_module_attribute_macro(token_stream: TokenStream) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();
    let (root_node, _diagnostics) = db.parse_virtual_with_diagnostics(token_stream);

    for n in root_node.descendants(&db) {
        // Process only the first module expected to be the contract.
        if n.kind(&db) == ItemModule {
            let module_ast = ast::ItemModule::from_syntax_node(&db, n);
            return from_module(&db, &module_ast);
        }
    }

    ProcMacroResult::new(TokenStream::empty())
}

pub fn from_module(db: &dyn SyntaxGroup, module_ast: &ast::ItemModule) -> ProcMacroResult {
    let name = module_ast.name(db).text(db);

    let mut diagnostics = vec![];

    diagnostics.extend(validate_attributes(db, &module_ast.attributes(db), DOJO_CONTRACT_ATTR));

    diagnostics.extend(validate_namings_diagnostics(&[("contract name", &name)]));

    let mut has_event = false;
    let mut has_storage = false;
    let mut has_init = false;
    let mut has_constructor = false;

    if let MaybeModuleBody::Some(body) = module_ast.body(db) {
        // TODO: Use `.iter_items_in_cfg(db, metadata.cfg_set)` when possible
        // to ensure we don't loop on items that are not in the current cfg set.
        let mut body_nodes: Vec<_> = body
            .items_vec(db)
            .iter()
            .flat_map(|el| {
                if let ast::ModuleItem::Enum(ref enum_ast) = el {
                    if enum_ast.name(db).text(db).to_string() == "Event" {
                        has_event = true;

                        return merge_event(db, enum_ast.clone());
                    }
                } else if let ast::ModuleItem::Struct(ref struct_ast) = el {
                    if struct_ast.name(db).text(db).to_string() == "Storage" {
                        has_storage = true;
                        return merge_storage(db, struct_ast.clone());
                    }
                } else if let ast::ModuleItem::FreeFunction(ref fn_ast) = el {
                    let fn_decl = fn_ast.declaration(db);
                    let fn_name = fn_decl.name(db).text(db);

                    if fn_name == CONSTRUCTOR_FN {
                        has_constructor = true;
                        return handle_constructor_fn(db, fn_ast);
                    }

                    if fn_name == DOJO_INIT_FN {
                        has_init = true;
                        return handle_init_fn(db, fn_ast, &mut diagnostics);
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
            body_nodes.append(&mut create_event())
        }

        if !has_storage {
            body_nodes.append(&mut create_storage())
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

        let (code, _) = builder.build();

        crate::debug_expand(&format!("CONTRACT PATCH: {name}"), &code);

        return ProcMacroResult::new(TokenStream::new(code))
            .with_diagnostics(Diagnostics::new(diagnostics));
    }

    ProcMacroResult::new(TokenStream::empty())
}
/// If a constructor is provided, we should keep the user statements.
/// We only inject the world provider initializer.
fn handle_constructor_fn(db: &dyn SyntaxGroup, fn_ast: &ast::FunctionWithBody) -> Vec<RewriteNode> {
    let fn_decl = fn_ast.declaration(db);

    let params_str = params_to_str(db, fn_decl.signature(db).parameters(db));

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
    db: &dyn SyntaxGroup,
    fn_ast: &ast::FunctionWithBody,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<RewriteNode> {
    let fn_decl = fn_ast.declaration(db);

    if let OptionReturnTypeClause::ReturnTypeClause(_) = fn_decl.signature(db).ret_ty(db) {
        diagnostics.push_error(format!("The {} function cannot have a return type.", DOJO_INIT_FN));
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
         core::panics::panic_with_byte_array(@format!(\"Only the world can init contract `{}`, \
         but caller is `{:?}`\", self.dojo_name(), starknet::get_caller_address())); }"
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

pub fn merge_event(db: &dyn SyntaxGroup, enum_ast: ast::ItemEnum) -> Vec<RewriteNode> {
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

pub fn create_event() -> Vec<RewriteNode> {
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

pub fn merge_storage(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> Vec<RewriteNode> {
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

pub fn create_storage() -> Vec<RewriteNode> {
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
pub fn params_to_str(db: &dyn SyntaxGroup, param_list: ast::ParamList) -> String {
    let params = param_list
        .elements(db)
        .iter()
        .map(|param| param.as_syntax_node().get_text(db))
        .collect::<Vec<_>>();

    params.join(", ")
}
