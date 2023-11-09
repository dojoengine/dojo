use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::{ItemEnum, ItemStruct};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

/// Derives PrintTrait for a struct.
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the model struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_print_struct(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> RewriteNode {
    let prints: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .map(|m| {
            format!(
                "debug::PrintTrait::print('{}'); debug::PrintTrait::print(self.{});",
                m.name(db).text(db).to_string(),
                m.name(db).text(db).to_string()
            )
        })
        .collect();

    RewriteNode::interpolate_patched(
        "#[cfg(test)]
            impl $type_name$StructPrintImpl of debug::PrintTrait<$type_name$> {
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

/// Derives PrintTrait for an enum.
/// Parameters:
/// * db: The semantic database.
/// * enum_ast: The AST of the model enum.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_print_enum(db: &dyn SyntaxGroup, enum_ast: ItemEnum) -> RewriteNode {
    let enum_name = enum_ast.name(db).text(db);
    let prints: Vec<_> = enum_ast
        .variants(db)
        .elements(db)
        .iter()
        .map(|m| {
            format!(
                "{}::{}(value) => {{ debug::PrintTrait::print('{}'); \
                 debug::PrintTrait::print(value); }}",
                enum_name,
                m.name(db).text(db).to_string(),
                m.name(db).text(db).to_string()
            )
        })
        .collect();

    RewriteNode::interpolate_patched(
        "#[cfg(test)]
            impl $type_name$EnumPrintImpl of debug::PrintTrait<$type_name$> {
                fn print(self: $type_name$) {
                    match self {
                        $print$
                    }
                }
            }",
        &UnorderedHashMap::from([
            ("type_name".to_string(), RewriteNode::new_trimmed(enum_ast.name(db).as_syntax_node())),
            ("print".to_string(), RewriteNode::Text(prints.join("\n"))),
        ]),
    )
}
