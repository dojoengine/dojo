use cairo_lang_macro::{derive_macro, ProcMacroResult, TokenStream};

use crate::helpers::debug_macro;

pub mod dojo_store;
pub mod introspect;

#[derive_macro]
pub fn introspect(token_stream: TokenStream) -> ProcMacroResult {
    let output = introspect::process(token_stream, false);

    debug_macro("Introspect", &output);
    output
}

#[derive_macro]
pub fn introspect_packed(token_stream: TokenStream) -> ProcMacroResult {
    let output = introspect::process(token_stream, true);

    debug_macro("IntrospectPacked", &output);
    output
}

#[derive_macro]
pub fn dojo_store(token_stream: TokenStream) -> ProcMacroResult {
    let output = dojo_store::process(token_stream);

    debug_macro("DojoStore", &output);
    output
}

#[derive_macro]
pub fn dojo_legacy_store(_token_stream: TokenStream) -> ProcMacroResult {
    // Nothing to do for DojoLegacyStore derive attribute because
    // it is directly handled through the dojo::model attribute.
    // But a derive_macro has to be defined to be able to use it as
    // derive attribute.
    ProcMacroResult::new(TokenStream::empty())
}
