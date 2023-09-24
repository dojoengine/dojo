use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_world::manifest::Member;
use itertools::Itertools;

/// A handler for Dojo code derives Introspect for a struct
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_introspect_struct(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> RewriteNode {
    let members: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .map(|member| Member {
            name: member.name(db).text(db).to_string(),
            ty: member.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string(),
            key: member.has_attr(db, "key"),
        })
        .collect::<_>();

    let layout: Vec<_> = members
        .iter()
        .filter_map(|m| {
            if m.key {
                return None;
            }

            Some(RewriteNode::Text(format!(
                "dojo::database::schema::SchemaIntrospection::<{}>::layout(ref layout);\n",
                m.ty
            )))
        })
        .collect::<_>();

    let member_types: Vec<_> = members
        .iter()
        .map(|m| {
            let mut attrs = vec![];
            if m.key {
                attrs.push("'key'")
            }

            format!(
                "dojo::database::schema::serialize_member(@dojo::database::schema::Member {{
				name: '{}',
				ty: dojo::database::schema::SchemaIntrospection::<{}>::ty(),
				attrs: array![{}].span()
			}})",
                m.name,
                m.ty,
                attrs.join(","),
            )
        })
        .collect::<_>();

    RewriteNode::interpolate_patched(
        "
		impl $name$SchemaIntrospection of dojo::database::schema::SchemaIntrospection<$name$> {
		   #[inline(always)]
		   fn size() -> usize {
			   $size$
		   }

		   #[inline(always)]
		   fn layout(ref layout: Array<u8>) {
			   $layout$
		   }

		   #[inline(always)]
		   fn ty() -> dojo::database::schema::Ty {
			   dojo::database::schema::Ty::Struct(dojo::database::schema::Struct {
				   name: '$name$',
				   attrs: array![].span(),
				   children: array![$member_types$].span()
			   })
		   }
	    }
        ",
        UnorderedHashMap::from([
            ("name".to_string(), RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node())),
            (
                "size".to_string(),
                RewriteNode::Text(
                    struct_ast
                        .members(db)
                        .elements(db)
                        .iter()
                        .filter_map(|member| {
                            if member.has_attr(db, "key") {
                                return None;
                            }

                            Some(format!(
                                "dojo::database::schema::SchemaIntrospection::<{}>::size()",
                                member.type_clause(db).ty(db).as_syntax_node().get_text(db),
                            ))
                        })
                        .join(" + "),
                ),
            ),
            ("layout".to_string(), RewriteNode::new_modified(layout)),
            ("member_types".to_string(), RewriteNode::Text(member_types.join(","))),
        ]),
    )
}
