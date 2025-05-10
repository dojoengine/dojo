use cairo_lang_macro::{inline_macro, ProcMacroResult, TokenStream};

mod bytearray_hash;
mod selector_from_tag;

#[inline_macro]
pub fn bytearray_hash(token_stream: TokenStream) -> ProcMacroResult {
    bytearray_hash::process(token_stream)
}

#[inline_macro]
pub fn selector_from_tag(token_stream: TokenStream) -> ProcMacroResult {
    selector_from_tag::process(token_stream)
}
