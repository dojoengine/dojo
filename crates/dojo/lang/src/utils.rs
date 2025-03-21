use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{Expr, ExprListParenthesized, Member as MemberAst};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedStablePtr, TypedSyntaxNode};
use dojo_types::naming::compute_bytearray_hash;
use starknet_crypto::{poseidon_hash_many, Felt};

use crate::aux_data::Member;

#[inline(always)]
pub fn get_serialization_path(use_serde: bool) -> String {
    if use_serde {
        "core::serde::Serde".to_string()
    } else {
        "dojo::storage::DojoStore".to_string()
    }
}

/// Compute a unique hash based on the element name and types and names of members.
/// This hash is used in element contracts to ensure uniqueness.
pub fn compute_unique_hash(
    db: &dyn SyntaxGroup,
    element_name: &str,
    is_packed: bool,
    members: &[MemberAst],
) -> Felt {
    let mut hashes =
        vec![if is_packed { Felt::ONE } else { Felt::ZERO }, compute_bytearray_hash(element_name)];
    hashes.extend(
        members
            .iter()
            .map(|m| {
                poseidon_hash_many(&[
                    compute_bytearray_hash(&m.name(db).text(db).to_string()),
                    compute_bytearray_hash(
                        m.type_clause(db).ty(db).as_syntax_node().get_text(db).trim(),
                    ),
                ])
            })
            .collect::<Vec<_>>(),
    );
    poseidon_hash_many(&hashes)
}

/// Parse the list of members from AST to build a list of `Member`.
pub fn parse_members(
    db: &dyn SyntaxGroup,
    members: &[MemberAst],
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> Vec<Member> {
    let mut parsing_keys = true;

    members
        .iter()
        .map(|member_ast| {
            let is_key = member_ast.has_attr(db, "key");

            let member = Member {
                name: member_ast.name(db).text(db).to_string(),
                ty: member_ast
                    .type_clause(db)
                    .ty(db)
                    .as_syntax_node()
                    .get_text(db)
                    .trim()
                    .to_string(),
                key: is_key,
            };

            // Make sure all keys are before values in the model.
            if is_key && !parsing_keys {
                diagnostics.push(PluginDiagnostic {
                    message: "Key members must be defined before non-key members.".into(),
                    stable_ptr: member_ast.name(db).stable_ptr().untyped(),
                    severity: Severity::Error,
                });
                // Don't return here, since we don't want to stop processing the members after the
                // first error to avoid diagnostics just because the field is
                // missing.
            }

            parsing_keys &= is_key;

            member
        })
        .collect::<Vec<_>>()
}

pub fn serialize_keys_and_values(
    db: &dyn SyntaxGroup,
    members: &[MemberAst],
    serialized_keys: &mut Vec<RewriteNode>,
    serialized_values: &mut Vec<RewriteNode>,
    use_serde: bool,
) {
    members.iter().for_each(|member| {
        let node = RewriteNode::Text(serialize_member_ty(db, member, true, use_serde));

        if member.has_attr(db, "key") {
            serialized_keys.push(node);
        } else {
            serialized_values.push(node);
        }
    });
}

pub fn deserialize_keys_and_values(
    db: &dyn SyntaxGroup,
    members: &[MemberAst],
    deserialized_keys: &mut Vec<RewriteNode>,
    deserialized_values: &mut Vec<RewriteNode>,
    use_serde: bool,
) {
    members.iter().for_each(|member| {
        let node = RewriteNode::Text(deserialize_member_ty(db, member, use_serde));

        if member.has_attr(db, "key") {
            deserialized_keys.push(node);
        } else {
            deserialized_values.push(node);
        }
    });
}

/// Destructure a tuple expression into:
/// - a string representing the destructured tuple like (e1,(e2, e3),e4),
/// - the list of tuple item types like ["u8", "u16", "u32"],
/// - the number of elements in the tuple.
///
/// Some examples:
/// (u8, u8, (u8, (u8, u8))) => ("(e1,e2,(e3,(e4,e5,),),)", ["u8", "u8", "u8", "u8", "u8"], 5).
/// (u8,(u16,u32,u64,),i8,) => ("(e1,(e2,e3,e4,),e5,)", ["u8", "u16", "u32", "u64", "i8"], 5)
pub fn destructure_tuple_member(
    db: &dyn SyntaxGroup,
    expr: &ExprListParenthesized,
    start: usize,
) -> (String, Vec<String>, usize) {
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
        format!(
            "({}){}",
            elements.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>().join(""),
            comma
        ),
        elements.iter().flat_map(|(_, t)| t.clone()).collect::<Vec<_>>(),
        current - 1,
    )
}

pub fn serialize_member_ty(
    db: &dyn SyntaxGroup,
    member_ast: &MemberAst,
    with_self: bool,
    use_serde: bool,
) -> String {
    let member_name = member_ast.name(db).text(db).to_string();
    match member_ast.type_clause(db).ty(db) {
        Expr::Tuple(expr) => {
            serialize_tuple_member_ty(db, &member_name, &expr, with_self, use_serde)
        }
        _ => serialize_primitive_member_ty(&member_name, with_self, use_serde),
    }
}

pub fn serialize_primitive_member_ty(
    member_name: &String,
    with_self: bool,
    use_serde: bool,
) -> String {
    let path = get_serialization_path(use_serde);

    format!(
        "{path}::serialize({}{member_name}, ref serialized);\n",
        if with_self { "self." } else { "" },
    )
}

pub fn serialize_tuple_member_ty(
    db: &dyn SyntaxGroup,
    member_name: &String,
    expr: &ExprListParenthesized,
    with_self: bool,
    use_serde: bool,
) -> String {
    // no serialization for unit tuple ()
    if expr.expressions(db).elements(db).is_empty() {
        return "".to_string();
    }

    let (tuple_repr, _, count) = destructure_tuple_member(db, expr, 1);
    let serialized_tuple_items = (1..count + 1)
        .map(|index| serialize_primitive_member_ty(&format!("e{index}"), false, use_serde))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "let {tuple_repr} = {}{member_name};
        {serialized_tuple_items}
        ",
        if with_self { "self." } else { "" }
    )
}

pub fn deserialize_member_ty(
    db: &dyn SyntaxGroup,
    member_ast: &MemberAst,
    use_serde: bool,
) -> String {
    let member_name = member_ast.name(db).text(db).to_string();

    match member_ast.type_clause(db).ty(db) {
        Expr::Tuple(expr) => deserialize_tuple_member_ty(db, &member_name, &expr, use_serde),
        _ => {
            let member_ty = member_ast.type_clause(db).ty(db).as_syntax_node().get_text(db);
            deserialize_primitive_member_ty(&member_name, &member_ty, use_serde)
        }
    }
}

pub fn deserialize_primitive_member_ty(
    member_name: &String,
    member_ty: &String,
    use_serde: bool,
) -> String {
    let path = get_serialization_path(use_serde);
    format!("let {member_name} = {path}::<{member_ty}>::deserialize(ref values)?;\n")
}

pub fn deserialize_tuple_member_ty(
    db: &dyn SyntaxGroup,
    member_name: &String,
    expr: &ExprListParenthesized,
    use_serde: bool,
) -> String {
    // no deserialization for unit tuple ()
    if expr.expressions(db).elements(db).is_empty() {
        return format!("let {member_name} = ();\n");
    }

    let (tuple_repr, tuple_types, _) = destructure_tuple_member(db, expr, 1);
    let deserialized_tuple_items = tuple_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            deserialize_primitive_member_ty(&format!("e{}", index + 1), ty, use_serde)
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        "{deserialized_tuple_items}
        let {member_name} = {tuple_repr};\n"
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
