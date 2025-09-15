use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{
    Expr, GenericParam, Member as MemberAst, OptionWrappedGenericParamList,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use itertools::Itertools;

use crate::helpers::get_serialization_path_and_prefix;

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
        let (path, prefix) = get_serialization_path_and_prefix(use_serde);

        format!(
            "{path}::{prefix}serialize({}{member_name}, ref serialized);\n",
            if with_self { "self." } else { "" },
        )
    }

    pub(crate) fn deserialize_member_ty(
        db: &dyn SyntaxGroup,
        member_ast: &MemberAst,
        use_serde: bool,
        input_name: &str,
    ) -> String {
        let member_name = member_ast.name(db).text(db).to_string();
        let member_ty = match member_ast.type_clause(db).ty(db) {
            Expr::Tuple(expr) => expr.as_syntax_node().get_text_without_trivia(db),
            _ => member_ast.type_clause(db).ty(db).as_syntax_node().get_text_without_trivia(db),
        };

        Self::deserialize_primitive_member_ty(&member_name, &member_ty, use_serde, input_name)
    }

    pub fn deserialize_primitive_member_ty(
        member_name: &str,
        member_ty: &str,
        use_serde: bool,
        input_name: &str,
    ) -> String {
        let (path, prefix) = get_serialization_path_and_prefix(use_serde);
        format!(
            "let {member_name} = {path}::<{member_ty}>::{prefix}deserialize(ref {input_name})?;\n"
        )
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

    // Extract generic type information and build the
    // type and impl information to add to the generated introspect
    pub fn build_generic_types(
        db: &SimpleParserDatabase,
        generic_params: OptionWrappedGenericParamList,
    ) -> Vec<String> {
        let generic_types = if let OptionWrappedGenericParamList::WrappedGenericParamList(params) =
            generic_params
        {
            params
                .generic_params(db)
                .elements(db)
                .filter_map(|el| {
                    if let GenericParam::Type(typ) = el {
                        Some(typ.name(db).text(db).to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        generic_types
    }

    pub fn build_generic_impls(
        gen_types: &[String],
        base_impls: &[String],
        additional_impls: &[String],
    ) -> String {
        let mut gen_impls = gen_types
            .iter()
            .map(|g| {
                format!(
                    "{g}, {base_impls}",
                    base_impls = base_impls.iter().map(|i| format!("{i}<{g}>")).join(", ")
                )
            })
            .collect::<Vec<_>>();

        if !gen_types.is_empty() {
            gen_impls.extend(additional_impls.to_vec());
        }

        gen_impls.join(", ")
    }
}
