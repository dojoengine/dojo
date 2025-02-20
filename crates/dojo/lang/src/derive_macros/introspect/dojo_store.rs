use cairo_lang_syntax::node::ast::{
    Expr, ExprListParenthesized, ItemEnum, ItemStruct, OptionTypeClause,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use crate::attribute_macros::element::{deserialize_member_ty, serialize_member_ty};

/// Destructure a tuple member into a string representing the destructured tuple,
/// and the index of the last element.
///
/// For example: (u8, u8, (u8, (u8, u8))) should give (e1,e2,(e3,(e4,e5,))) and 5.
fn destructure_tuple_member(
    db: &dyn SyntaxGroup,
    expr: &ExprListParenthesized,
    start: usize,
) -> (String, usize) {
    if expr.expressions(db).elements(db).is_empty() {
        return ("()".to_string(), 0);
    }

    let elements = expr
        .expressions(db)
        .elements(db)
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let current = start + index;
            match element {
                Expr::Tuple(expr) => destructure_tuple_member(db, expr, current),
                _ => (format!("e{},", current), current),
            }
        })
        .collect::<Vec<_>>();

    (
        format!("({})", elements.iter().map(|(str, _)| str.clone()).collect::<Vec<_>>().join("")),
        elements.last().unwrap().1,
    )
}

/// Generate the list of tuple element deserialization.
///
/// For example: (u8, u16, (u32, (u64, u128))) should give:
/// let e1 = dojo::storage::DojoStore::<u8>::deserialize(ref values)?;
/// let e2 = dojo::storage::DojoStore::<u16>::deserialize(ref values)?;
/// let e3 = dojo::storage::DojoStore::<u32>::deserialize(ref values)?;
/// let e4 = dojo::storage::DojoStore::<u64>::deserialize(ref values)?;
/// let e5 = dojo::storage::DojoStore::<u128>::deserialize(ref values)?;
fn deserialize_tuple_list(
    db: &dyn SyntaxGroup,
    expr: &ExprListParenthesized,
    start: usize,
) -> Vec<String> {
    expr.expressions(db)
        .elements(db)
        .iter()
        .enumerate()
        .flat_map(|(index, element)| {
            let current = start + index;
            match element {
                Expr::Tuple(expr) => deserialize_tuple_list(db, expr, current),
                Expr::Path(p) => {
                    let ty = p.as_syntax_node().get_text(db);
                    vec![format!(
                        "let e{} = dojo::storage::DojoStore::<{ty}>::deserialize(ref values)?;",
                        current
                    )]
                }
                // TODO RBA: handle Expr::FixedSizeArray
                _ => {
                    unimplemented!(
                        "Tuple: Expr '{}' not supported inside tuples",
                        element.as_syntax_node().get_text(db)
                    )
                }
            }
        })
        .collect::<Vec<_>>()
}

/// Generate the list of tuple element serialization.
fn serialize_tuple_list(last: usize) -> Vec<String> {
    (1..last + 1)
        .map(|index| format!("dojo::storage::DojoStore::serialize(e{index}, ref serialized);"))
        .collect::<Vec<_>>()
}

/// Serialize a tuple member by destructure it and then call DojoStore::serialize for
/// every tuple items.
fn serialize_tuple_member(
    db: &dyn SyntaxGroup,
    member_name: &String,
    expr: &ExprListParenthesized,
) -> String {
    if expr.expressions(db).elements(db).is_empty() {
        return "".to_string();
    }

    let (tuple_repr, last) = destructure_tuple_member(db, expr, 1);
    let serialized_tuple_items = serialize_tuple_list(last).join("\n");

    format!(
        "let {tuple_repr} = self.{member_name};
        {serialized_tuple_items}
        "
    )
}

/// Deserialize a tuple member by deserialize every tuple items and then rebuild
/// the original tuple.
fn deserialize_tuple_member(
    db: &dyn SyntaxGroup,
    member_name: &String,
    expr: &ExprListParenthesized,
) -> String {
    let (tuple_repr, _) = destructure_tuple_member(db, expr, 1);
    let deserialized_tuple_items = deserialize_tuple_list(db, expr, 1).join("\n");

    format!(
        "{deserialized_tuple_items}
        let {member_name} = {tuple_repr};"
    )
}

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

        match member.type_clause(db).ty(db) {
            Expr::Tuple(tuple) => {
                serialized_members.push(serialize_tuple_member(db, &member_name, &tuple));
                deserialized_members.push(deserialize_tuple_member(db, &member_name, &tuple));
            }
            _ => {
                serialized_members.push(serialize_member_ty(&member_name, true, false));
                deserialized_members.push(deserialize_member_ty(&member_name, &member_ty, false));
            }
        }

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

        let (serialized_variant, deserialized_variant) = match variant.type_clause(db) {
            OptionTypeClause::TypeClause(ty) => match ty.ty(db) {
                Expr::Tuple(expr) => {
                    let (tuple_repr, last) = destructure_tuple_member(db, &expr, 1);
                    let serialized_tuple_items = serialize_tuple_list(last).join("\n");
                    let deserialized_tuple_items = deserialize_tuple_list(db, &expr, 1).join("\n");

                    let serialized = format!(
                        "{full_variant_name}(d) => {{
                                serialized.append({variant_index});
                                let {tuple_repr} = d;
                                {serialized_tuple_items}
                            }},"
                    );

                    let deserialized = format!(
                        "{variant_index} => {{
                                {deserialized_tuple_items}
                                let variant_data = {tuple_repr};
                                Option::Some({full_variant_name}(variant_data))
                            }},",
                    );

                    (serialized, deserialized)
                }
                _ => {
                    let ty = ty.ty(db).as_syntax_node().get_text(db).trim().to_string();

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
