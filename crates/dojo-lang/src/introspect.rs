use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_world::manifest::Member;

#[derive(Clone, Default)]
struct TypeIntrospection(usize, Vec<usize>);

fn primitive_type_introspection() -> HashMap<String, TypeIntrospection> {
    HashMap::from([
        ("felt252".into(), TypeIntrospection(1, vec![251])),
        ("bool".into(), TypeIntrospection(1, vec![1])),
        ("u8".into(), TypeIntrospection(1, vec![8])),
        ("u16".into(), TypeIntrospection(1, vec![16])),
        ("u32".into(), TypeIntrospection(1, vec![32])),
        ("u64".into(), TypeIntrospection(1, vec![64])),
        ("u128".into(), TypeIntrospection(1, vec![128])),
        ("u256".into(), TypeIntrospection(2, vec![128, 128])),
        ("usize".into(), TypeIntrospection(1, vec![32])),
        ("ContractAddress".into(), TypeIntrospection(1, vec![251])),
        ("ClassHash".into(), TypeIntrospection(1, vec![251])),
    ])
}

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

    let primitive_sizes = primitive_type_introspection();
    let mut size_precompute = 0;

    let mut size = vec![];
    let mut layout = vec![];
    let mut member_types = vec![];

    members.iter().for_each(|m| {
        let primitive_intro = primitive_sizes.get(&m.ty);
        let mut attrs = vec![];

        if let Some(p_ty) = primitive_intro {
            // It's a primitive type
            if m.key {
                attrs.push("'key'");
            } else {
                size_precompute += p_ty.0;
                p_ty.1.iter().for_each(|l| {
                    layout.push(RewriteNode::Text(format!("layout.append({});\n", l)))
                });
            }
            // Do this for both keys and non keys
            member_types.push(format!(
                "dojo::database::schema::serialize_member(@dojo::database::schema::Member {{
                name: '{}',
                ty: dojo::database::schema::Ty::Primitive('{}'),
                attrs: array![{}].span()
            }})",
                m.name,
                m.ty,
                attrs.join(","),
            ));
        } else {
            // It's a custom type
            if m.key {
                attrs.push("'key'");
            } else {
                size.push(format!(
                    "dojo::database::schema::SchemaIntrospection::<{}>::size()",
                    m.ty,
                ));
                layout.push(RewriteNode::Text(format!(
                    "dojo::database::schema::SchemaIntrospection::<{}>::layout(ref layout);\n",
                    m.ty
                )));
            }
            // Do this for both keys and non keys
            member_types.push(format!(
                "dojo::database::schema::serialize_member(@dojo::database::schema::Member {{
                name: '{}',
                ty: dojo::database::schema::SchemaIntrospection::<{}>::ty(),
                attrs: array![{}].span()
            }})",
                m.name,
                m.ty,
                attrs.join(","),
            ));
        }
    });

    if size_precompute > 0 {
        size.push(format!("{}", size_precompute));
    }

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
            ("size".to_string(), RewriteNode::Text(size.join(" + "))),
            ("layout".to_string(), RewriteNode::new_modified(layout)),
            ("member_types".to_string(), RewriteNode::Text(member_types.join(","))),
        ]),
    )
}
