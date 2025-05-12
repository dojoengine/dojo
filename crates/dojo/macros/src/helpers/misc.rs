use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::Member;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use dojo_types::naming;
use starknet_crypto::{poseidon_hash_many, Felt};

/// Compute a unique hash based on the element name and types and names of members.
/// This hash is used in element contracts to ensure uniqueness.
pub fn compute_unique_hash(
    db: &SimpleParserDatabase,
    element_name: &str,
    is_packed: bool,
    members: &[Member],
) -> Felt {
    let mut hashes = vec![
        if is_packed { Felt::ONE } else { Felt::ZERO },
        naming::compute_bytearray_hash(element_name),
    ];
    hashes.extend(
        members
            .iter()
            .map(|m| {
                poseidon_hash_many(&[
                    naming::compute_bytearray_hash(&m.name(db).text(db)),
                    naming::compute_bytearray_hash(
                        &m.type_clause(db).ty(db).as_syntax_node().get_text_without_trivia(db),
                    ),
                ])
            })
            .collect::<Vec<_>>(),
    );
    poseidon_hash_many(&hashes)
}
