/// Represents a member of a struct.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub ty: String,
    pub key: bool,
}

pub mod debug;
pub use debug::*;

pub mod misc;
pub use misc::*;

pub mod checker;
pub use checker::*;

pub mod parser;
pub use parser::*;

pub mod formatter;
pub use formatter::*;

pub mod tokenizer;
pub use tokenizer::*;

pub mod diagnostic_ext;
pub use diagnostic_ext::*;

pub mod proc_macro_result_ext;
pub use proc_macro_result_ext::*;
