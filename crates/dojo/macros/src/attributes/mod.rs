use crate::helpers::debug_macro;
use cairo_lang_macro::{attribute_macro, ProcMacroResult, TokenStream};

pub mod contract;
pub mod event;
pub mod library;
pub mod model;

#[attribute_macro(parent = "dojo")]
pub fn model(_args: TokenStream, token_stream: TokenStream) -> ProcMacroResult {
    let output = model::DojoModel::process(token_stream);

    debug_macro("model", &output);
    output
}

#[attribute_macro(parent = "dojo")]
pub fn event(_args: TokenStream, token_stream: TokenStream) -> ProcMacroResult {
    let output = event::DojoEvent::process(token_stream);

    debug_macro("event", &output);
    output
}

#[attribute_macro(parent = "dojo")]
pub fn contract(_args: TokenStream, token_stream: TokenStream) -> ProcMacroResult {
    let output = contract::DojoContract::process(token_stream);

    debug_macro("contract", &output);
    output
}

#[attribute_macro(parent = "dojo")]
pub fn library(_args: TokenStream, token_stream: TokenStream) -> ProcMacroResult {
    let output = library::DojoLibrary::process(token_stream);

    debug_macro("library", &output);
    output
}
