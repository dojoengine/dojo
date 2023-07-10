use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::TypedSyntaxNode;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use itertools::Itertools;

/// A handler for Dojo code derives SerdeLen for a struct
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_serde_len_struct(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> RewriteNode {
    RewriteNode::interpolate_patched(
        "
        impl SerdeLen$name$ of dojo::SerdeLen<$type$> {
            #[inline(always)]
            fn len() -> usize {
                $len$
            }
        }
        ",
        UnorderedHashMap::from([
            ("name".to_string(), RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node())),
            ("type".to_string(), RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node())),
            (
                "len".to_string(),
                RewriteNode::Text(
                    struct_ast
                        .members(db)
                        .elements(db)
                        .iter()
                        .map(|member| {
                            format!(
                                "dojo::SerdeLen::<{}>::len()",
                                member.type_clause(db).ty(db).as_syntax_node().get_text(db),
                            )
                        })
                        .join(" + "),
                ),
            ),
        ]),
    )
}
