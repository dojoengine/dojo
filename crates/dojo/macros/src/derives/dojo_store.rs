use cairo_lang_macro::{quote, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::OptionTypeClause;
use cairo_lang_syntax::node::kind::SyntaxKind::{ItemEnum, ItemStruct};
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

use crate::helpers::{debug_store_expand, DojoFormatter, DojoTokenizer, ProcMacroResultExt};

pub(crate) fn process(token_stream: TokenStream) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();
    let (root_node, _diagnostics) = db.parse_token_stream(&token_stream);

    for n in root_node.descendants(&db) {
        match n.kind(&db) {
            ItemStruct => {
                let struct_ast = ast::ItemStruct::from_syntax_node(&db, n);
                return process_struct(&db, &struct_ast);
            }
            ItemEnum => {
                let enum_ast = ast::ItemEnum::from_syntax_node(&db, n);
                return process_enum(&db, &enum_ast);
            }
            _ => {}
        }
    }

    ProcMacroResult::fail("derive DojoStore: unsupported syntax node.".to_string())
}

fn process_struct(db: &SimpleParserDatabase, struct_ast: &ast::ItemStruct) -> ProcMacroResult {
    let struct_name = struct_ast.name(db).text(db).to_string();
    let gen_types = DojoFormatter::build_generic_types(db, struct_ast.generic_params(db));

    let gen_impls = DojoFormatter::build_generic_impls(
        &gen_types,
        &["+dojo::storage::DojoStore".to_string()],
        &[],
    );

    let mut serialized_members = vec![];
    let mut deserialized_members = vec![];
    let mut member_names = vec![];

    for member in struct_ast.members(db).elements(db).iter() {
        let member_name = member.name(db).text(db).to_string();

        let member_ty = member.type_clause(db).ty(db).as_syntax_node().get_text_without_trivia(db);

        serialized_members.push(DojoFormatter::serialize_primitive_member_ty(
            &member_name,
            true,
            false,
        ));
        deserialized_members.push(DojoFormatter::deserialize_primitive_member_ty(
            &member_name,
            &member_ty,
            false,
            "values",
        ));

        member_names.push(member_name);
    }

    let serialized_members = serialized_members.join("");
    let deserialized_members = deserialized_members.join("");
    let member_names = member_names.join(",\n");

    let generic_params =
        if gen_types.is_empty() { "".to_string() } else { format!("<{}>", gen_types.join(", ")) };

    let impl_decl = if gen_types.is_empty() {
        format!("impl {struct_name}DojoStore of dojo::storage::DojoStore<{struct_name}>")
    } else {
        format!(
            "impl {struct_name}DojoStore<{gen_impls}> of \
             dojo::storage::DojoStore<{struct_name}{generic_params}>"
        )
    };

    let dojo_store = format!(
        "{impl_decl} {{
        fn dojo_serialize(self: @{struct_name}{generic_params}, ref serialized: Array<felt252>) {{
            {serialized_members}
        }}
        fn dojo_deserialize(ref values: Span<felt252>) -> Option<{struct_name}{generic_params}> {{
            {deserialized_members}
            Option::Some({struct_name}{} {{
                {member_names}
            }})
        }}
    }}",
        if gen_types.is_empty() { "".to_string() } else { format!("::{generic_params}") }
    );

    debug_store_expand(&format!("DOJO_STORE STRUCT::{struct_name}"), &dojo_store);

    let dojo_store = DojoTokenizer::tokenize(&dojo_store);

    ProcMacroResult::new(quote! {
        #dojo_store
    })
}

fn process_enum(db: &SimpleParserDatabase, enum_ast: &ast::ItemEnum) -> ProcMacroResult {
    let enum_name = enum_ast.name(db).text(db).to_string();
    let gen_types = DojoFormatter::build_generic_types(db, enum_ast.generic_params(db));

    let enum_name_with_generics = format!("{enum_name}<{}>", gen_types.join(", "));

    let gen_impls = DojoFormatter::build_generic_impls(
        &gen_types,
        &["+dojo::storage::DojoStore".to_string(), "+core::serde::Serde".to_string()],
        &[format!("+core::traits::Default<{enum_name_with_generics}>")],
    );

    let mut serialized_variants = vec![];
    let mut deserialized_variants = vec![];

    for (index, variant) in enum_ast.variants(db).elements(db).iter().enumerate() {
        let variant_name = variant.name(db).text(db).to_string();
        let full_variant_name = format!("{enum_name}::{variant_name}");
        let variant_index = index + 1;

        let (serialized_variant, deserialized_variant) = match variant.type_clause(db) {
            OptionTypeClause::TypeClause(ty) => {
                let ty = ty.ty(db).as_syntax_node().get_text_without_trivia(db);

                let serialized = format!(
                    "{full_variant_name}(d) => {{
                            serialized.append({variant_index});
                            dojo::storage::DojoStore::dojo_serialize(d, ref serialized);
                        }},"
                );

                let deserialized = format!(
                    "{variant_index} => {{
                            let variant_data = \
                     dojo::storage::DojoStore::<{ty}>::dojo_deserialize(ref values)?;
                            Option::Some({full_variant_name}(variant_data))
                        }},",
                );

                (serialized, deserialized)
            }
            OptionTypeClause::Empty(_) => {
                let serialized =
                    format!("{full_variant_name} => {{ serialized.append({variant_index}); }},");
                let deserialized =
                    format!("{variant_index} => Option::Some({full_variant_name}),",);

                (serialized, deserialized)
            }
        };

        serialized_variants.push(serialized_variant);
        deserialized_variants.push(deserialized_variant);
    }

    let serialized_variants = serialized_variants.join("\n");
    let deserialized_variants = deserialized_variants.join("\n");

    let generic_params =
        if gen_types.is_empty() { "".to_string() } else { format!("<{}>", gen_types.join(", ")) };

    let impl_decl = if gen_types.is_empty() {
        format!("impl {enum_name}DojoStore of dojo::storage::DojoStore<{enum_name}>")
    } else {
        format!(
            "impl {enum_name}DojoStore<{gen_impls}> of \
             dojo::storage::DojoStore<{enum_name}{generic_params}>"
        )
    };

    let dojo_store = format!(
        "{impl_decl} {{
        fn dojo_serialize(self: @{enum_name}{generic_params}, ref serialized: Array<felt252>) {{
            match self {{
                {serialized_variants}
            }};
        }}
        fn dojo_deserialize(ref values: Span<felt252>) -> Option<{enum_name}{generic_params}> {{
            let variant = *values.pop_front()?;
            match variant {{
                0 => Option::Some(Default::<{enum_name}{generic_params}>::default()),
                {deserialized_variants}
                _ => Option::None,
            }}
        }}
    }}"
    );

    debug_store_expand(&format!("DOJO_STORE ENUM::{enum_name}"), &dojo_store);

    let dojo_store = DojoTokenizer::tokenize(&dojo_store);

    ProcMacroResult::new(quote! {
        #dojo_store
    })
}
