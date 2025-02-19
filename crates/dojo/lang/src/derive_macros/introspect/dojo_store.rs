use cairo_lang_syntax::node::ast::{ItemEnum, ItemStruct, OptionTypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use crate::attribute_macros::element::{deserialize_member_ty, serialize_member_ty};

pub fn build_struct_dojo_store(
    db: &dyn SyntaxGroup,
    name: &String,
    struct_ast: &ItemStruct,
) -> String {
    let mut serialized_members = vec![];
    let mut deserialized_members = vec![];
    let mut member_names = vec![];

    for member in struct_ast.members(db).elements(db).iter() {
        let member_name = member.name(db).text(db).to_string();
        let member_ty =
            member.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string();

        serialized_members.push(serialize_member_ty(&member_name, true, false));
        deserialized_members.push(deserialize_member_ty(&member_name, &member_ty, false));
        member_names.push(member_name);
    }

    let serialized_members = serialized_members.join("");
    let deserialized_members = deserialized_members.join("");
    let member_names = member_names.join(",\n");

    format!(
        "impl {name}DojoStore of dojo::storage::DojoStore<{name}> {{
        fn serialize(self: @{name}, ref serialized: Array<felt252>) {{
            {serialized_members}
        }}
        fn deserialize(ref values: Span<felt252>) -> Option<{name}> {{
            {deserialized_members}
            Option::Some({name} {{
                {member_names}
            }})
        }}
    }}"
    )
}

pub fn build_enum_dojo_store(db: &dyn SyntaxGroup, name: &String, enum_ast: &ItemEnum) -> String {
    let mut serialized_variants = vec![];
    let mut deserialized_variants = vec![];

    for (index, variant) in enum_ast.variants(db).elements(db).iter().enumerate() {
        let variant_name = variant.name(db).text(db).to_string();
        let full_variant_name = format!("{name}::{variant_name}");
        let variant_index = index + 1;

        let serialized_variant = match variant.type_clause(db) {
            OptionTypeClause::TypeClause(_) => {
                format!(
                    "{full_variant_name}(d) => {{
                        serialized.append({variant_index});
                        dojo::storage::DojoStore::serialize(d, ref serialized);
                    }},"
                )
            }
            OptionTypeClause::Empty(_) => {
                format!("{full_variant_name} => {{ serialized.append({variant_index}); }},")
            }
        };

        let deserialized_variant = match variant.type_clause(db) {
            OptionTypeClause::TypeClause(ty) => {
                let ty = ty.ty(db).as_syntax_node().get_text(db).trim().to_string();
                format!(
                    "{variant_index} => {{
                        let variant_data = dojo::storage::DojoStore::<{ty}>::deserialize(ref \
                     values)?;
                        Option::Some({full_variant_name}(variant_data))
                    }},",
                )
            }
            OptionTypeClause::Empty(_) => {
                format!("{variant_index} => Option::Some({full_variant_name}),",)
            }
        };

        serialized_variants.push(serialized_variant);
        deserialized_variants.push(deserialized_variant);
    }

    let serialized_variants = serialized_variants.join("\n");
    let deserialized_variants = deserialized_variants.join("\n");

    format!(
        "impl {name}DojoStore of dojo::storage::DojoStore<{name}> {{
        fn serialize(self: @{name}, ref serialized: Array<felt252>) {{
            match self {{
                {serialized_variants}
            }};
        }}
        fn deserialize(ref values: Span<felt252>) -> Option<{name}> {{
            let variant = *values.pop_front()?;

            match variant {{
                0 => Option::Some(Default::<{name}>::default()),
                {deserialized_variants}
                _ => Option::None,
            }}
        }}
    }}"
    )
}
