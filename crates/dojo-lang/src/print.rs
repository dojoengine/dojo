use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

/// Derives PrintTrait for a struct.
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the model struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn derive_print(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> RewriteNode {
    let prints: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .map(|m| {
            format!(
                "core::debug::PrintTrait::print('{}'); core::debug::PrintTrait::print(self.{});",
                m.name(db).text(db).to_string(),
                m.name(db).text(db).to_string()
            )
        })
        .collect();

    RewriteNode::interpolate_patched(
        "#[cfg(test)]
            impl $type_name$PrintImpl of core::debug::PrintTrait<$type_name$> {
                fn print(self: $type_name$) {
                    $print$
                }
            }",
        &UnorderedHashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("print".to_string(), RewriteNode::Text(prints.join("\n"))),
        ]),
    )
}
