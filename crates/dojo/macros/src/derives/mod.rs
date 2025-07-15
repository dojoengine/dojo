use cairo_lang_macro::{derive_macro, ProcMacroResult, TokenStream};

use crate::helpers::debug_macro;

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
pub fn dojo_legacy_storage(_token_stream: TokenStream) -> ProcMacroResult {
    // Nothing to do for DojoLegacyStorage derive attribute because
    // it is directly handled through the dojo::model attribute.
    // But a derive_macro has to be defined to be able to use it as
    // derive attribute.
    ProcMacroResult::new(TokenStream::empty())
}
