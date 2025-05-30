use cairo_lang_syntax::node::ast::{Expr, ExprListParenthesized, Member as MemberAst};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use crate::helpers::get_serialization_path;

pub struct DojoFormatter {}

/// DojoFormatter provides some functions to format data structure
/// to be used in output token streams.
impl DojoFormatter {
    /// Return member declaration statement from member name and type.
    pub(crate) fn get_member_declaration(name: &str, ty: &str) -> String {
        format!("pub {}: {},\n", name, ty)
    }

    pub(crate) fn serialize_member_ty(
        db: &dyn SyntaxGroup,
        member_ast: &MemberAst,
        with_self: bool,
        use_serde: bool,
    ) -> String {
        let member_name = member_ast.name(db).text(db).to_string();
        match member_ast.type_clause(db).ty(db) {
            Expr::Tuple(expr) => {
                Self::serialize_tuple_member_ty(db, &member_name, &expr, with_self, use_serde)
            }
            _ => Self::serialize_primitive_member_ty(&member_name, with_self, use_serde),
        }
    }

    pub(crate) fn serialize_primitive_member_ty(
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

    pub(crate) fn serialize_tuple_member_ty(
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

        let (tuple_repr, _, count) = Self::destructure_tuple_member(db, expr, 1);
        let serialized_tuple_items = (1..count + 1)
            .map(|index| {
                Self::serialize_primitive_member_ty(&format!("e{index}"), false, use_serde)
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "let {tuple_repr} = {}{member_name};
            {serialized_tuple_items}
            ",
            if with_self { "self." } else { "" }
        )
    }

    pub(crate) fn deserialize_member_ty(
        db: &dyn SyntaxGroup,
        member_ast: &MemberAst,
        use_serde: bool,
    ) -> String {
        let member_name = member_ast.name(db).text(db).to_string();

        match member_ast.type_clause(db).ty(db) {
            Expr::Tuple(expr) => {
                Self::deserialize_tuple_member_ty(db, &member_name, &expr, use_serde)
            }
            _ => {
                let member_ty = member_ast.type_clause(db).ty(db).as_syntax_node().get_text(db);
                Self::deserialize_primitive_member_ty(&member_name, &member_ty, use_serde)
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

        let (tuple_repr, tuple_types, _) = Self::destructure_tuple_member(db, expr, 1);
        let deserialized_tuple_items = tuple_types
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                Self::deserialize_primitive_member_ty(&format!("e{}", index + 1), ty, use_serde)
            })
            .collect::<Vec<_>>()
            .join("");

        format!(
            "{deserialized_tuple_items}
            let {member_name} = {tuple_repr};\n"
        )
    }

    pub fn serialize_keys_and_values(
        db: &dyn SyntaxGroup,
        members: &[MemberAst],
        serialized_keys: &mut Vec<String>,
        serialized_values: &mut Vec<String>,
        use_serde: bool,
    ) {
        members.iter().for_each(|member| {
            let serialized = Self::serialize_member_ty(db, member, true, use_serde);

            if member.has_attr(db, "key") {
                serialized_keys.push(serialized);
            } else {
                serialized_values.push(serialized);
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
                    if expr.as_syntax_node().get_text_without_trivia(db) == "()" {
                        ("(),".to_string(), vec![])
                    } else {
                        let (tuple_repr, tuple_types, index) =
                            Self::destructure_tuple_member(db, expr, current);
                        current = index + 1;
                        (tuple_repr, tuple_types)
                    }
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
           test8: ((), (u8, ())),
           test9: (u8, (u8, (), u8)),
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
            ("((),(e1,(),))", vec!["u8"], 1_usize),
            ("(e1,(e2,(),e3,),)", vec!["u8", "u8", "u8"], 3_usize),
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
                    let (tuple_repr, tuple_types, last) =
                        DojoFormatter::destructure_tuple_member(&db, &tuple, 1);

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
