use cairo_lang_syntax::node::ast::{Expr, Member as MemberAst};
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
        Self::serialize_primitive_member_ty(&member_name, with_self, use_serde)
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

    pub(crate) fn deserialize_member_ty(
        db: &dyn SyntaxGroup,
        member_ast: &MemberAst,
        use_serde: bool,
    ) -> String {
        let member_name = member_ast.name(db).text(db).to_string();
        let member_ty = match member_ast.type_clause(db).ty(db) {
            Expr::Tuple(expr) => expr.as_syntax_node().get_text_without_all_comment_trivia(db),
            _ => member_ast
                .type_clause(db)
                .ty(db)
                .as_syntax_node()
                .get_text_without_all_comment_trivia(db),
        };

        Self::deserialize_primitive_member_ty(&member_name, &member_ty, use_serde)
    }

    pub fn deserialize_primitive_member_ty(
        member_name: &String,
        member_ty: &String,
        use_serde: bool,
    ) -> String {
        let path = get_serialization_path(use_serde);
        format!("let {member_name} = {path}::<{member_ty}>::deserialize(ref values)?;\n")
    }

    pub fn serialize_keys_and_values(
        db: &dyn SyntaxGroup,
        members: impl Iterator<Item = MemberAst>,
        serialized_keys: &mut Vec<String>,
        serialized_values: &mut Vec<String>,
        use_serde: bool,
    ) {
        members.for_each(|member| {
            let serialized = Self::serialize_member_ty(db, &member, true, use_serde);

            if member.has_attr(db, "key") {
                serialized_keys.push(serialized);
            } else {
                serialized_values.push(serialized);
            }
        });
    }
}
