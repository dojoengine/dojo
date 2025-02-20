use cairo_lang_syntax::node::ast::{
    Expr, ExprListParenthesized, ItemEnum, ItemStruct, OptionTypeClause,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use itertools::Itertools;

use crate::attribute_macros::element::{deserialize_member_ty, serialize_member_ty};

/// Destructure a tuple member into a string representing the destructured tuple,
/// the list tuple item types, and the index of the last element.
///
/// Some examples:
/// (u8, u8, (u8, (u8, u8))) => ("(e1,e2,(e3,(e4,e5,),),)", ["u8", "u8", "u8", "u8", "u8"], 5).
/// (u8,(u16,u32,u64,),i8,) => ("(e1,(e2,e3,e4,),e5,)", ["u8", "u16", "u32", "u64", "i8"], 5)
fn destructure_tuple_member(
    db: &dyn SyntaxGroup,
    expr: &ExprListParenthesized,
    start: usize,
) -> (String, Vec<String>, usize) {
    if expr.expressions(db).elements(db).is_empty() {
        return ("()".to_string(), vec![], 0);
    }

    let mut current = start;

    let elements = expr
        .expressions(db)
        .elements(db)
        .iter()
        .map(|element| match element {
            Expr::Tuple(expr) => {
                let (tuple_repr, tuple_types, index) = destructure_tuple_member(db, expr, current);
                current = index + 1;
                (tuple_repr, tuple_types)
            }
            Expr::Path(p) => {
                let str = format!("e{},", current);
                current += 1;
                (str, vec![p.as_syntax_node().get_text(db)])
            }
            _ => {
                unimplemented!(
                    "Tuple: Expr '{}' not supported inside tuples",
                    element.as_syntax_node().get_text(db)
                )
            }
        })
        .collect::<Vec<_>>();

    let comma = if start == 1 { "" } else { "," };
    (
        format!("({}){}", elements.iter().map(|(s, _)| s).join(""), comma),
        elements.iter().flat_map(|(_, t)| t.clone()).collect::<Vec<_>>(),
        current - 1,
    )
}

/// Generate the list of tuple element deserialization.
///
/// For example: ["u8", "u16", "u32", "u64", "u128"] should give:
/// let e1 = dojo::storage::DojoStore::<u8>::deserialize(ref values)?;
/// let e2 = dojo::storage::DojoStore::<u16>::deserialize(ref values)?;
/// ...
/// let e5 = dojo::storage::DojoStore::<u128>::deserialize(ref values)?;
fn deserialize_tuple_list(tuple_types: &[String]) -> Vec<String> {
    tuple_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            format!("let e{index} = dojo::storage::DojoStore::<{ty}>::deserialize(ref values)?;",)
        })
        .collect()
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

    let (tuple_repr, _, last) = destructure_tuple_member(db, expr, 1);
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
    let (tuple_repr, tuple_types, _) = destructure_tuple_member(db, expr, 1);
    let deserialized_tuple_items = deserialize_tuple_list(&tuple_types).join("\n");

    format!(
        "{deserialized_tuple_items}
        let {member_name} = {tuple_repr};\n"
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
                    let (tuple_repr, tuple_types, last) = destructure_tuple_member(db, &expr, 1);
                    let serialized_tuple_items = serialize_tuple_list(last).join("\n");
                    let deserialized_tuple_items = deserialize_tuple_list(&tuple_types).join("\n");

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

#[cfg(test)]
mod tests {
    use cairo_lang_parser::utils::SimpleParserDatabase;
    use cairo_lang_syntax::node::ast;
    use cairo_lang_syntax::node::kind::SyntaxKind::ItemStruct;

    use super::*;

    #[test]
    fn test_destructure_tuple_member() {
        let db = SimpleParserDatabase::default();

        let input = r#"
        struct S {
           test1: (),
           test2: (u8, u16, u32),
           test3: (u8, (u16, u32)),
           test4: (u8, (u16,), u32),
           test5: (u8, (u16, u128, u256), u32),
           test6: (u8, u16, (u32, (u64, u128))),
           test7: (MyEnum, u8),
        }
        "#;
        let expected = [
            ("()", vec![], 0_usize),
            ("(e1,e2,e3,)", vec!["u8", "u16", "u32"], 3_usize),
            ("(e1,(e2,e3,),)", vec!["u8", "u16", "u32"], 3_usize),
            ("(e1,(e2,),e3,)", vec!["u8", "u16", "u32"], 3_usize),
            ("(e1,(e2,e3,e4,),e5,)", vec!["u8", "u16", "u128", "u256", "u32"], 5_usize),
            ("(e1,e2,(e3,(e4,e5,),),)", vec!["u8", "u16", "u32", "u64", "u128"], 5_usize),
            ("(e1,e2,)", vec!["MyEnum", "u8"], 2_usize),
        ];

        let root_node = db.parse_virtual(input).expect("code: parsing failed");
        for n in root_node.descendants(&db) {
            if n.kind(&db) == ItemStruct {
                let test_struct = ast::ItemStruct::from_syntax_node(&db, n);

                for (index, (member, (expected_repr, expected_types, expected_last))) in
                    test_struct.members(&db).elements(&db).iter().zip(expected.iter()).enumerate()
                {
                    let test_id = format!("test_{}", index + 1);

                    let tuple = match member.type_clause(&db).ty(&db) {
                        Expr::Tuple(t) => t,
                        _ => panic!("Unexpected data type for this test"),
                    };
                    let (tuple_repr, tuple_types, last) = destructure_tuple_member(&db, &tuple, 1);

                    assert_eq!(
                        tuple_repr,
                        expected_repr.to_string(),
                        "{test_id}: tuple representation error"
                    );
                    assert_eq!(
                        tuple_types,
                        expected_types.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                        "{test_id}: tuple types error"
                    );
                    assert_eq!(last, *expected_last, "{test_id}: tuple last error");
                }
            }
        }
    }
}
