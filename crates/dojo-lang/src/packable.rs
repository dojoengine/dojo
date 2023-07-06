use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use itertools::Itertools;

/// A handler for Dojo code that modifies a packable struct.
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the packable struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_packable_struct(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> RewriteNode {
    RewriteNode::interpolate_patched(
        "
        impl Packable$name$ of dojo::Packable<$type$> {
            #[inline(always)]
            fn pack(self: @$type$, ref packing: felt252, ref packing_offset: u8, ref packed: \
         Array<felt252>) {
                $pack_members$
            }
            #[inline(always)]
            fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) \
         -> Option<$type$> {
                option::Option::Some($type$ {
                    $unpack_members$
                })
            }
            #[inline(always)]
            fn size(self: @$type$) -> usize {
                $size_members$
            }
        }
        ",
        UnorderedHashMap::from([
            ("name".to_string(), RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node())),
            ("type".to_string(), RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node())),
            (
                "pack_members".to_string(),
                RewriteNode::new_modified(
                    struct_ast
                        .members(db)
                        .elements(db)
                        .iter()
                        .map(|member| {
                            RewriteNode::interpolate_patched(
                                "
                                dojo::Packable::pack(self.$name$, ref packing, ref packing_offset, \
                                 ref packed);",
                                UnorderedHashMap::from([(
                                    "name".to_string(),
                                    RewriteNode::new_trimmed(member.name(db).as_syntax_node()),
                                )]),
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "unpack_members".to_string(),
                RewriteNode::new_modified(
                    struct_ast
                        .members(db)
                        .elements(db)
                        .iter()
                        .map(|member| {
                            RewriteNode::interpolate_patched(
                                "
                                $name$: \
                                 option::OptionTrait::unwrap(dojo::Packable::<$type$>::unpack(
                                    ref packed,
                                    ref unpacking,
                                    ref unpacking_offset
                                )),",
                                UnorderedHashMap::from([
                                    (
                                        "type".to_string(),
                                        RewriteNode::new_trimmed(
                                            member.type_clause(db).ty(db).as_syntax_node(),
                                        ),
                                    ),
                                    (
                                        "name".to_string(),
                                        RewriteNode::new_trimmed(member.name(db).as_syntax_node()),
                                    ),
                                ]),
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "size_members".to_string(),
                RewriteNode::Text(
                    struct_ast
                        .members(db)
                        .elements(db)
                        .iter()
                        .map(|member| {
                            format!("dojo::Packable::size(self.{})", member.name(db).text(db))
                        })
                        .join(" + "),
                ),
            ),
        ]),
    )
}
