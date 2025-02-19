use cairo_lang_syntax::node::ast::{Expr, ItemEnum, ItemStruct, OptionTypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use crate::derive_macros::introspect::utils::clean_ty;
use crate::utils::{
    deserialize_primitive_member_ty, deserialize_tuple_member_ty, serialize_primitive_member_ty,
    serialize_tuple_member_ty,
};

pub fn build_struct_dojo_store(
    db: &dyn SyntaxGroup,
    name: &String,
    struct_ast: &ItemStruct,
    generic_types: &[String],
    generic_impls: &String,
) -> String {
    let mut serialized_members = vec![];
    let mut deserialized_members = vec![];
    let mut member_names = vec![];

    for member in struct_ast.members(db).elements(db).iter() {
        let member_name = member.name(db).text(db).to_string();

        let member_ty =
            member.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string();

        // TODO: no more useful once this issue will be fixed:
        // https://github.com/starkware-libs/cairo/issues/7706
        let member_ty = clean_ty(&member_ty);

        match member.type_clause(db).ty(db) {
            Expr::Tuple(tuple) => {
                serialized_members.push(serialize_tuple_member_ty(
                    db,
                    &member_name,
                    &tuple,
                    true,
                    false,
                ));
                deserialized_members.push(deserialize_tuple_member_ty(
                    db,
                    &member_name,
                    &tuple,
                    false,
                ));
            }
            _ => {
                serialized_members.push(serialize_primitive_member_ty(&member_name, true, false));
                deserialized_members.push(deserialize_primitive_member_ty(
                    &member_name,
                    &member_ty,
                    false,
                ));
            }
        }

        member_names.push(member_name);
    }

    let serialized_members = serialized_members.join("");
    let deserialized_members = deserialized_members.join("");
    let member_names = member_names.join(",\n");

    let generic_types = generic_types.join(", ");

    format!(
        "impl {name}DojoStore<{generic_impls}> of \
         dojo::storage::DojoStore<{name}<{generic_types}>> {{
        fn serialize(self: @{name}<{generic_types}>, ref serialized: Array<felt252>) {{
            {serialized_members}
        }}
        fn deserialize(ref values: Span<felt252>) -> Option<{name}<{generic_types}>> {{
            {deserialized_members}
            Option::Some({name}::<{generic_types}> {{
                {member_names}
            }})
        }}
    }}"
    )
}

pub fn build_enum_dojo_store(
    db: &dyn SyntaxGroup,
    name: &String,
    enum_ast: &ItemEnum,
    generic_types: &[String],
    generic_impls: &String,
) -> String {
    let mut serialized_variants = vec![];
    let mut deserialized_variants = vec![];

    for (index, variant) in enum_ast.variants(db).elements(db).iter().enumerate() {
        let variant_name = variant.name(db).text(db).to_string();
        let full_variant_name = format!("{name}::{variant_name}");
        let variant_index = index + 1;

        let (serialized_variant, deserialized_variant) = match variant.type_clause(db) {
            OptionTypeClause::TypeClause(ty) => match ty.ty(db) {
                Expr::Tuple(expr) => {
                    let serialized_tuple =
                        serialize_tuple_member_ty(db, &"d".to_string(), &expr, false, false);
                    let deserialized_tuple =
                        deserialize_tuple_member_ty(db, &"variant_data".to_string(), &expr, false);

                    let serialized = format!(
                        "{full_variant_name}(d) => {{
                                serialized.append({variant_index});
                                {serialized_tuple}
                            }},"
                    );

                    let deserialized = format!(
                        "{variant_index} => {{
                                {deserialized_tuple}
                                Option::Some({full_variant_name}(variant_data))
                            }},",
                    );

                    (serialized, deserialized)
                }
                _ => {
                    let ty = ty.ty(db).as_syntax_node().get_text(db).trim().to_string();

                    // TODO: no more useful once this issue will be fixed:
                    // https://github.com/starkware-libs/cairo/issues/7706
                    let ty = clean_ty(&ty);

                    let serialized = format!(
                        "{full_variant_name}(d) => {{
                                serialized.append({variant_index});
                                dojo::storage::DojoStore::serialize(d, ref serialized);
                            }},"
                    );

                    let deserialized = format!(
                        "{variant_index} => {{
                                let variant_data = \
                         dojo::storage::DojoStore::<{ty}>::deserialize(ref values)?;
                                Option::Some({full_variant_name}(variant_data))
                            }},",
                    );

                    (serialized, deserialized)
                }
            },
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

    let generic_types = generic_types.join(", ");

    format!(
        "impl {name}DojoStore<{generic_impls}> of \
         dojo::storage::DojoStore<{name}<{generic_types}>> {{
        fn serialize(self: @{name}<{generic_types}>, ref serialized: Array<felt252>) {{
            match self {{
                {serialized_variants}
            }};
        }}
        fn deserialize(ref values: Span<felt252>) -> Option<{name}<{generic_types}>> {{
            let variant = *values.pop_front()?;

            match variant {{
                0 => Option::Some(Default::<{name}<{generic_types}>>::default()),
                {deserialized_variants}
                _ => Option::None,
            }}
        }}
    }}"
    )
}
