use std::collections::HashMap;
use std::sync::Mutex;

use cairo_lang_syntax::node::ast::{ExprBlock, ExprPath, ExprStructCtorCall};
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::SyntaxNode;

type ModuleName = String;
type FunctionName = String;
lazy_static::lazy_static! {
    pub static ref WRITERS: Mutex<HashMap<ModuleName, HashMap<FunctionName, Vec<WriterLookupDetails>>>> = Default::default();
}

pub enum WriterLookupDetails {
    StructCtor(ExprStructCtorCall),
    Path(ExprPath, ExprBlock),
}

pub fn get_parent_block(
    db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
    target: &SyntaxNode,
    kind: SyntaxKind,
) -> Option<SyntaxNode> {
    let mut new_target = target.clone();
    while let Some(parent) = new_target.parent() {
        if kind == parent.kind(db) {
            return Some(parent);
        }
        new_target = parent;
    }
    None
}
